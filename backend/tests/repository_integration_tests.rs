use chrono::Utc;
use dotenv; // Added import for dotenv
use fyp_portal::{
    models::{CreateProjectRequest, Project, UpdateProjectRequest, User},
    repository::{PostgresRepository, Repository},
};
use sqlx::PgPool;
use tokio::test;
use uuid::Uuid;

// --- Test Context and Setup ---

/// A simple structure to hold the database pool for testing
struct DbTestContext {
    pool: PgPool,
}

impl DbTestContext {
    async fn setup() -> Self {
        dotenv::dotenv().ok();

        let db_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set to run integration tests");

        let pool = PgPool::connect(&db_url)
            .await
            .expect("Failed to connect to database for integration tests.");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run database migrations.");

        DbTestContext { pool }
    }

    fn repository(&self) -> PostgresRepository {
        PostgresRepository::new(self.pool.clone())
    }
}

// --- Test Data Helpers ---

/// Inserts a mock user into BOTH auth.users and public.profiles.
async fn create_test_user(pool: &PgPool, id: Uuid, role: &str) -> User {
    let email = format!("{}@test.com", role);

    // Use a CTE to ensure both inserts happen atomically
    let created_user = sqlx::query_as!(
        User,
        r#"
        WITH auth_user AS (
            INSERT INTO auth.users (id, email) 
            VALUES ($1, $2)
            ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email
            RETURNING id, email
        )
        INSERT INTO public.profiles (id, email, role) 
        SELECT id, email, $3 FROM auth_user
        ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email, role = EXCLUDED.role
        RETURNING id, email, role
        "#,
        id,
        email,
        role
    )
    .fetch_one(pool)
    .await
    .expect("Failed to create test user");

    created_user
}

/// Inserts a mock project into the database directly.
async fn create_test_project(
    pool: &PgPool,
    user_id: Uuid,
    title: &str,
    year: i32,
    is_public: bool,
) -> Project {
    let project_uuid = Uuid::new_v4();
    let author_name = "Test Author";
    let abstract_text = "Test Abstract";
    let cover_key = "cover_image_key";
    let video_key: Option<String> = None; // Explicitly set type for Option binding
    let report_key: Option<String> = None; // Explicitly set type for Option binding
    let report_pub = false;
    let created = Utc::now();
    let updated = Utc::now();

    sqlx::query_as!(
        Project,
        r#"INSERT INTO public.projects (
             id, user_id, author, title, abstract, cover_image, 
             video, report, 
             year, is_public, report_is_public, created_at, updated_at
           )
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
           RETURNING 
             id, user_id, author, title, abstract as abstract_text, cover_image, 
             video, report, 
             is_public, report_is_public, year, created_at, updated_at"#,
        // --- 13 PARAMETERS LISTED HERE ---
        project_uuid,    // $1: id (Uuid)
        user_id,         // $2: user_id (Uuid)
        author_name,     // $3: author (&str)
        title,           // $4: title (&str)
        abstract_text,   // $5: abstract (&str)
        cover_key,       // $6: cover_image (&str)
        video_key as _,  // $7: video (Option<String>)
        report_key as _, // $8: report (Option<String>)
        year,            // $9: year (i32)
        is_public,       // $10: is_public (bool)
        report_pub,      // $11: report_is_public (bool)
        created,         // $12: created_at (DateTime<Utc>)
        updated,         // $13: updated_at (DateTime<Utc>)
    )
    // REMOVE all .bind() calls after the macro
    .fetch_one(pool)
    .await
    .expect("Failed to create test project")
}

// --- Tests ---

#[test]
async fn test_create_and_get_project() {
    let ctx = DbTestContext::setup().await;
    let repo = ctx.repository();
    let user = create_test_user(&ctx.pool, Uuid::new_v4(), "student").await;

    let req = CreateProjectRequest {
        title: "Test Project Title".to_string(),
        abstract_text: "A brief summary".to_string(),
        author_name: "Test User".to_string(),
        year: 2024,
        cover_image_key: "key1".to_string(),
        video_key: None,
        report_key: None,
    };

    // 1. Test Create
    let created_project = repo.create_project(req.clone(), user.id).await;
    assert_eq!(created_project.title, req.title);
    assert_eq!(created_project.user_id, user.id);
    assert!(
        !created_project.is_public,
        "Projects should be private by default"
    );

    // 2. Test Get
    let fetched_project = repo.get_project(created_project.id).await;
    assert!(fetched_project.is_some());
    assert_eq!(fetched_project.unwrap().title, req.title);
}

#[test]
async fn test_get_projects_with_filters() {
    let ctx = DbTestContext::setup().await;
    let repo = ctx.repository();
    let user = create_test_user(&ctx.pool, Uuid::new_v4(), "student").await;

    // Create test data
    create_test_project(&ctx.pool, user.id, "Rust Backend", 2024, true).await;
    create_test_project(&ctx.pool, user.id, "Go Frontend", 2023, true).await;
    create_test_project(&ctx.pool, user.id, "Search Rust Query", 2024, true).await;
    create_test_project(&ctx.pool, user.id, "Hidden Project", 2024, false).await; // Private

    // Test 1: No filter (Should only return public projects)
    let all_projects = repo.get_projects(None, None).await;
    let our_projects: Vec<_> = all_projects
        .iter()
        .filter(|p| p.user_id == user.id)
        .collect();
    assert_eq!(
        our_projects.len(),
        3,
        "Should find 3 public projects for this user"
    );

    // Test 2: Filter by year (2024)
    let year_projects = repo.get_projects(Some(2024), None).await;
    let our_2024: Vec<_> = year_projects
        .iter()
        .filter(|p| p.user_id == user.id)
        .collect();
    assert_eq!(
        our_2024.len(),
        2,
        "Should find 2 projects from 2024 for this user"
    );

    // Test 3: Filter by search term ("Rust")
    let search_projects = repo.get_projects(None, Some("Rust".to_string())).await;
    let our_rust: Vec<_> = search_projects
        .iter()
        .filter(|p| p.user_id == user.id)
        .collect();
    assert_eq!(
        our_rust.len(),
        2,
        "Should find 2 projects with 'Rust' for this user"
    );

    // Test 4: Filter by year and search
    let filtered_projects = repo
        .get_projects(Some(2024), Some("Backend".to_string()))
        .await;
    let our_filtered: Vec<_> = filtered_projects
        .iter()
        .filter(|p| p.user_id == user.id)
        .collect();
    assert_eq!(
        our_filtered.len(),
        1,
        "Should find 1 project matching both filters for this user"
    );
}

#[test]
async fn test_update_and_delete_project_ownership() {
    let ctx = DbTestContext::setup().await;
    let repo = ctx.repository();
    let owner = create_test_user(&ctx.pool, Uuid::new_v4(), "owner").await;
    let non_owner = create_test_user(&ctx.pool, Uuid::new_v4(), "nonowner").await;
    let project = create_test_project(&ctx.pool, owner.id, "To Update", 2023, false).await;

    // Test 1: Update by Non-Owner (Should fail)
    let update_req = UpdateProjectRequest {
        title: Some("New Title".to_string()),
        abstract_text: None,
        cover_image_key: None,
        video_key: None,
        report_key: None,
    };
    let updated_project_fail = repo
        .update_project(project.id, non_owner.id, update_req.clone())
        .await;
    assert!(
        updated_project_fail.is_none(),
        "Non-owner should not be able to update."
    );

    // Test 2: Update by Owner (Should succeed)
    let updated_project_success = repo.update_project(project.id, owner.id, update_req).await;
    assert!(updated_project_success.is_some());
    assert_eq!(updated_project_success.unwrap().title, "New Title");

    // Test 3: Delete by Non-Owner (Should fail)
    let delete_fail = repo.delete_project(project.id, non_owner.id).await;
    assert!(!delete_fail, "Non-owner should not be able to delete.");

    // Test 4: Delete by Owner (Should succeed)
    let delete_success = repo.delete_project(project.id, owner.id).await;
    assert!(delete_success, "Owner should be able to delete.");

    // Verify deletion
    let deleted_project = repo.get_project(project.id).await;
    assert!(deleted_project.is_none());
}

#[test]
async fn test_comment_lifecycle_and_deletion() {
    let ctx = DbTestContext::setup().await;
    let repo = ctx.repository();
    let user = create_test_user(&ctx.pool, Uuid::new_v4(), "commenter").await;
    let _admin = create_test_user(&ctx.pool, Uuid::new_v4(), "admin").await;
    let project = create_test_project(&ctx.pool, user.id, "Comment Test", 2024, true).await;

    // 1. Add comment
    let comment_text = "This is a great project!";
    let comment = repo
        .add_comment(project.id, user.id, comment_text.to_string())
        .await;
    assert_eq!(comment.comment, comment_text);

    // 2. Retrieve comments
    let comments = repo.get_comments(project.id).await;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].author_email.as_ref().unwrap(), &user.email);

    // 3. Delete by non-owner/non-admin (Should fail)
    let other_user = create_test_user(&ctx.pool, Uuid::new_v4(), "other").await;
    let delete_fail = repo.delete_comment(comment.id, other_user.id).await;
    assert!(!delete_fail);

    // 4. Delete by Admin (Should succeed via admin path)
    let delete_success_admin = repo.delete_comment_admin(comment.id).await;
    assert!(delete_success_admin);

    // Verify deletion
    let comments_after_delete = repo.get_comments(project.id).await;
    assert!(comments_after_delete.is_empty());
}

#[test]
async fn test_notification_and_read_status() {
    let ctx = DbTestContext::setup().await;
    let repo = ctx.repository();
    let recipient = create_test_user(&ctx.pool, Uuid::new_v4(), "recipient").await;
    let actor = create_test_user(&ctx.pool, Uuid::new_v4(), "actor").await;
    let project = create_test_project(&ctx.pool, recipient.id, "Notif Project", 2024, true).await;

    // Directly insert a notification (simulating a complex trigger like a comment)
    let notification_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO public.notifications (id, user_id, actor_id, project_id, type, is_read, created_at) 
          VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(notification_id)
    .bind(recipient.id)
    .bind(actor.id)
    .bind(project.id)
    .bind("comment")
    .bind(false)
    .bind(Utc::now())
    .execute(&ctx.pool)
    .await
    .expect("Failed to create test notification");

    // 1. Get notifications
    let notifs = repo.get_notifications(recipient.id).await;
    assert_eq!(notifs.len(), 1);
    assert!(!notifs[0].is_read);
    assert_eq!(notifs[0].project_title, project.title);
    assert_eq!(notifs[0].actor_email, actor.email);

    // 2. Mark as read
    let mark_success = repo
        .mark_notification_read(notification_id, recipient.id)
        .await;
    assert!(mark_success);

    // 3. Verify read status (direct SQL check)
    let is_read: bool =
        sqlx::query_scalar("SELECT is_read FROM public.notifications WHERE id = $1")
            .bind(notification_id)
            .fetch_one(&ctx.pool)
            .await
            .expect("Failed to fetch notification read status");

    assert!(is_read);
}
