use crate::models::{AdminDashboardStats, CreateProjectRequest, Project, User, Like, Comment, UpdateProjectRequest};
use async_trait::async_trait;
use sqlx::{PgPool, query_builder::QueryBuilder};
use uuid::Uuid;
use std::sync::Arc;

/// Repository Trait
///
/// Defines the abstract contract for all persistence operations. This is the core
/// of the Repository Abstraction pattern, allowing the handlers to interact with
/// the data layer without knowing the specific implementation (Postgres, Mock, etc.).
///
/// **Send + Sync + async_trait** are required to make the trait object (`Arc<dyn Repository>`)
/// safely shareable and usable across Axum's asynchronous task boundaries.
#[async_trait]
pub trait Repository: Send + Sync {
    // --- Project Retrieval ---
    // Public listing with filtering. Must enforce is_public=true.
    async fn get_projects(&self, year: Option<i32>, search: Option<String>) -> Vec<Project>;
    // Admin access: retrieves all projects regardless of status.
    async fn get_all_projects(&self) -> Vec<Project>;
    // Retrieves top projects ranked by like count.
    async fn get_top_projects(&self, limit: i64) -> Vec<Project>;

    // Retrieval methods with specific visibility and authorization rules.
    async fn get_project(&self, id: Uuid) -> Option<Project>;
    async fn get_project_authorized(&self, id: Uuid, user_id: Uuid) -> Option<Project>;
    async fn get_public_project(&self, id: Uuid) -> Option<Project>;

    // --- Project Actions ---
    async fn create_project(&self, req: CreateProjectRequest, user_id: Uuid) -> Project;
    // Idempotent operation: returns true if a row was inserted, false otherwise (conflict).
    async fn like_project(&self, like: Like) -> bool; 
    // Admin action: changes the is_public status.
    async fn set_project_status(&self, id: Uuid, is_public: bool) -> Option<Project>;

    // --- User/Auth ---
    async fn get_user(&self, id: Uuid) -> Option<User>;
    async fn create_user(&self, user: User) -> User;
    async fn get_stats(&self) -> AdminDashboardStats;
    
    // --- Owner Actions ---
    async fn get_my_projects(&self, user_id: Uuid) -> Vec<Project>;
    // Owner-Only: Deletes only if the user_id matches the project's user_id.
    async fn delete_project(&self, id: Uuid, user_id: Uuid) -> bool; 
    // Owner-Only: Updates only if the user_id matches. Uses COALESCE for partial updates.
    async fn update_project(&self, id: Uuid, user_id: Uuid, req: UpdateProjectRequest) -> Option<Project>;
    
    // --- Comments & Moderation ---
    async fn add_comment(&self, project_id: Uuid, user_id: Uuid, text: String) -> Comment;
    async fn get_comments(&self, project_id: Uuid) -> Vec<Comment>;

    /// Admin Override: Delete ANY project by ID (No ownership check).
    async fn delete_project_admin(&self, id: Uuid) -> bool;
    
    /// User: Delete their OWN comment (Ownership check required).
    async fn delete_comment(&self, id: i64, user_id: Uuid) -> bool;
    
    /// Admin: Delete ANY comment (No ownership check).
    async fn delete_comment_admin(&self, id: i64) -> bool;

    // --- Notifications ---
    // Retrieves enriched notification responses for the recipient (user_id).
    async fn get_notifications(&self, user_id: Uuid) -> Vec<crate::models::NotificationResponse>;
    // Marks a notification as read, enforced by ownership check (`user_id`).
    async fn mark_notification_read(&self, notification_id: Uuid, user_id: Uuid) -> bool;
}

/// RepositoryState
///
/// The concrete type used to share the persistence layer access across the application state.
pub type RepositoryState = Arc<dyn Repository>;

/// PostgresRepository
///
/// The concrete implementation of the `Repository` trait, backed by the PostgreSQL database.
pub struct PostgresRepository {
    pool: PgPool,
}

impl PostgresRepository {
    /// Creates a new repository instance using the initialized connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Repository for PostgresRepository {

    /// get_projects
    ///
    /// Implements flexible search/filtering using QueryBuilder for safe parameterization,
    /// adhering to the **"No SQL Injection Risk"** mandate.
    /// **Security**: Strictly enforces `WHERE is_public = true` in the base query.
    async fn get_projects(&self, year: Option<i32>, search: Option<String>) -> Vec<Project> {
        let mut builder: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            r#"
            SELECT 
                id, user_id, author, title, abstract, 
                cover_image, video, report, is_public, report_is_public, 
                year, created_at, updated_at 
            FROM projects 
            WHERE is_public = true 
            "#
        );
        
        if let Some(y) = year {
            builder.push(" AND year = ");
            builder.push_bind(y);
        }
        
        if let Some(s) = search {
            // Case-insensitive search across title, abstract, and author fields.
            let search_pattern = format!("%{}%", s);
            builder.push(" AND (title ILIKE ");
            builder.push_bind(search_pattern.clone());
            builder.push(" OR abstract ILIKE ");
            builder.push_bind(search_pattern.clone());
            builder.push(" OR author ILIKE ");
            builder.push_bind(search_pattern);
            builder.push(")");
        }
        
        builder.push(" ORDER BY created_at DESC");
        
        let query = builder.build_query_as::<Project>();
        
        match query.fetch_all(&self.pool).await {
            Ok(p) => p,
            Err(e) => { 
                tracing::error!("get_projects error: {:?}", e); 
                vec![] 
            }
        }
    }

    /// get_all_projects
    ///
    /// Administrative function to retrieve all project records.
    /// **Note**: Does *not* include the `WHERE is_public = true` restriction.
    async fn get_all_projects(&self) -> Vec<Project> {
        match sqlx::query_as!(Project, 
            r#"SELECT id, user_id, author, title, abstract as abstract_text, cover_image, video, report, is_public, report_is_public, year, created_at, updated_at FROM projects ORDER BY is_public ASC, created_at DESC"#
        ).fetch_all(&self.pool).await {
            Ok(p) => p,
            Err(e) => { tracing::error!("get_all_projects error: {:?}", e); vec![] }
        }
    }
    
    /// get_top_projects
    ///
    /// Retrieves projects by a ranking based on the number of likes.
    /// **Security**: Enforces `WHERE p.is_public = true`.
    async fn get_top_projects(&self, limit: i64) -> Vec<Project> {
        match sqlx::query_as!(
            Project,
            r#"SELECT p.id, p.user_id, p.author, p.title, p.abstract as abstract_text, p.cover_image, p.video, p.report, p.is_public, p.report_is_public, p.year, p.created_at, p.updated_at FROM projects p LEFT JOIN project_likes l ON p.id = l.project_id WHERE p.is_public = true GROUP BY p.id ORDER BY COUNT(l.user_id) DESC LIMIT $1"#,
            limit
        ).fetch_all(&self.pool).await {
            Ok(p) => p,
            Err(e) => { tracing::error!("get_top_projects error: {:?}", e); vec![] }
        }
    }

    /// get_project
    ///
    /// Simple retrieval of any project by ID (no visibility check). Primarily for internal use
    /// when visibility has already been determined by the calling handler (e.g., admin).
    async fn get_project(&self, id: Uuid) -> Option<Project> {
        sqlx::query_as!(Project, 
            r#"SELECT id, user_id, author, title, abstract as abstract_text, 
                      cover_image, video, report, is_public, report_is_public, 
                      year, created_at, updated_at 
                FROM projects 
                WHERE id = $1"#,
            id)
        .fetch_optional(&self.pool).await.unwrap_or_else(|e| { 
            tracing::error!("get_project error: {:?}", e); 
            None 
        })
    }

    /// get_project_authorized
    ///
    /// Retrieves a project if it is public OR if the querying user is the owner.
    async fn get_project_authorized(&self, id: Uuid, user_id: Uuid) -> Option<Project> {
        sqlx::query_as!(Project, 
            r#"SELECT id, user_id, author, title, abstract as abstract_text, 
                      cover_image, video, report, is_public, report_is_public, 
                      year, created_at, updated_at 
                FROM projects 
                WHERE id = $1 AND (is_public = true OR user_id = $2)"#,
            id, user_id)
        .fetch_optional(&self.pool).await.unwrap_or_else(|e| { 
            tracing::error!("get_project_authorized error: {:?}", e); 
            None 
        })
    }

    /// get_public_project
    ///
    /// Retrieves a project *only* if it is marked as public. Used by the public detail handler.
    async fn get_public_project(&self, id: Uuid) -> Option<Project> {
        sqlx::query_as!(Project, 
            r#"SELECT id, user_id, author, title, abstract as abstract_text, 
                      cover_image, video, report, is_public, report_is_public, 
                      year, created_at, updated_at 
                FROM projects 
                WHERE id = $1 AND is_public = true"#, 
            id)
        .fetch_optional(&self.pool).await.unwrap_or_else(|e| { 
            tracing::error!("get_public_project error: {:?}", e); 
            None 
        })
    }

    /// create_project
    ///
    /// Inserts a new project. All new projects are set to `is_public = false` by default,
    /// requiring administrative approval.
    async fn create_project(&self, req: CreateProjectRequest, user_id: Uuid) -> Project {
        let new_id = Uuid::new_v4();
        sqlx::query_as!(
            Project,
            r#"INSERT INTO projects (id, user_id, author, title, abstract, cover_image, video, report, year, is_public, report_is_public, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, false, false, NOW(), NOW()) RETURNING id, user_id, author, title, abstract as abstract_text, cover_image, video, report, is_public, report_is_public, year, created_at, updated_at"#,
            new_id, user_id, req.author_name, req.title, req.abstract_text, req.cover_image_key, req.video_key, req.report_key, req.year
        ).fetch_one(&self.pool).await.expect("Failed to insert project")
    }

    /// like_project
    ///
    /// Inserts a project like. Uses `ON CONFLICT DO NOTHING` to ensure **idempotency**.
    /// The function returns true only if a new row was inserted (`rows_affected > 0`).
    async fn like_project(&self, like: Like) -> bool {
        let result = sqlx::query!("INSERT INTO project_likes (user_id, project_id) VALUES ($1, $2) ON CONFLICT DO NOTHING", like.user_id, like.project_id).execute(&self.pool).await;
        match result { 
            Ok(res) => res.rows_affected() > 0, 
            Err(e) => { 
                // A true conflict (double vote) does not error, only database errors are caught here.
                tracing::error!("like error: {:?}", e); 
                false 
            } 
        }
    }

    /// set_project_status
    ///
    /// Updates the `is_public` flag. Used by the admin status update handler.
    async fn set_project_status(&self, id: Uuid, is_public: bool) -> Option<Project> {
        sqlx::query_as!(Project, r#"UPDATE projects SET is_public = $1 WHERE id = $2 RETURNING id, user_id, author, title, abstract as abstract_text, cover_image, video, report, is_public, report_is_public, year, created_at, updated_at"#, is_public, id)
        .fetch_optional(&self.pool).await.unwrap_or_else(|e| { tracing::error!("status error: {:?}", e); None })
    }

    /// get_user
    ///
    /// Retrieves user profile data (ID, email, role) needed for authentication and authorization.
    async fn get_user(&self, id: Uuid) -> Option<User> {
        sqlx::query_as!(User, "SELECT id, email, role FROM profiles WHERE id = $1", id).fetch_optional(&self.pool).await.unwrap_or(None)
    }

    /// create_user
    ///
    /// Creates the mirroring profile record in `public.profiles` after external auth success.
    async fn create_user(&self, user: User) -> User {
        sqlx::query_as!(User, "INSERT INTO profiles (id, email, role) VALUES ($1, $2, $3) RETURNING id, email, role", user.id, user.email, user.role).fetch_one(&self.pool).await.expect("Failed to create user")
    }

    /// get_stats
    ///
    /// Compiles all necessary counters for the administrative dashboard in a single call.
    async fn get_stats(&self) -> AdminDashboardStats {
        let total_projects = sqlx::query_scalar!("SELECT COUNT(*) FROM projects").fetch_one(&self.pool).await.unwrap_or(Some(0)).unwrap_or(0);
        let total_users = sqlx::query_scalar!("SELECT COUNT(*) FROM profiles").fetch_one(&self.pool).await.unwrap_or(Some(0)).unwrap_or(0);
        let total_likes = sqlx::query_scalar!("SELECT COUNT(*) FROM project_likes").fetch_one(&self.pool).await.unwrap_or(Some(0)).unwrap_or(0);
        let pending_reviews = sqlx::query_scalar!("SELECT COUNT(*) FROM projects WHERE is_public = false").fetch_one(&self.pool).await.unwrap_or(Some(0)).unwrap_or(0);
        AdminDashboardStats { total_projects, total_users, total_likes, pending_reviews }
    }

    // --- OWNER ACTIONS ---

    /// get_my_projects
    ///
    /// Retrieves all projects owned by the authenticated user, including unapproved/hidden ones.
    async fn get_my_projects(&self, user_id: Uuid) -> Vec<Project> {
        match sqlx::query_as!(Project, r#"SELECT id, user_id, author, title, abstract as abstract_text, cover_image, video, report, is_public, report_is_public, year, created_at, updated_at FROM projects WHERE user_id = $1 ORDER BY created_at DESC"#, user_id).fetch_all(&self.pool).await {
            Ok(p) => p,
            Err(e) => { tracing::error!("get_my_projects error: {:?}", e); vec![] }
        }
    }

    /// delete_project
    ///
    /// Deletes a project only if the provided `user_id` matches the project owner.
    /// This is the **Owner-Only** authorization check.
    async fn delete_project(&self, id: Uuid, user_id: Uuid) -> bool {
        match sqlx::query!("DELETE FROM projects WHERE id = $1 AND user_id = $2", id, user_id).execute(&self.pool).await {
            Ok(res) => res.rows_affected() > 0,
            Err(e) => { tracing::error!("delete error: {:?}", e); false }
        }
    }

    /// update_project
    ///
    /// Updates a project only if the provided `user_id` matches the owner.
    /// Uses the PostgreSQL `COALESCE` function to efficiently handle `Option<T>` fields,
    /// only updating a column if the corresponding field in `req` is `Some`.
    async fn update_project(&self, id: Uuid, user_id: Uuid, req: UpdateProjectRequest) -> Option<Project> {
        sqlx::query_as!(
            Project,
            r#"
            UPDATE projects 
            SET title = COALESCE($3, title),
                abstract = COALESCE($4, abstract),
                cover_image = COALESCE($5, cover_image),
                video = COALESCE($6, video),
                report = COALESCE($7, report),
                updated_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING id, user_id, author, title, abstract as abstract_text, 
                      cover_image, video, report, is_public, report_is_public, 
                      year, created_at, updated_at
            "#,
            id, user_id,
            req.title, req.abstract_text, req.cover_image_key, req.video_key, req.report_key
        )
        .fetch_optional(&self.pool)
        .await
        .unwrap_or_else(|e| { tracing::error!("update error: {:?}", e); None })
    }
    
    // --- COMMENT ACTIONS ---

    /// add_comment
    ///
    /// Inserts a new comment and immediately joins with `profiles` to return the enriched
    /// `Comment` model, including the author's email.
    async fn add_comment(&self, project_id: Uuid, user_id: Uuid, text: String) -> Comment {
        // Uses a CTE (Common Table Expression) to perform the insert and subsequent join in one query.
        let rec = sqlx::query!(
            r#"
            WITH inserted AS (
                INSERT INTO project_comments (project_id, user_id, comment) VALUES ($1, $2, $3) RETURNING id, user_id, project_id, comment, created_at
            )
            SELECT i.id, i.user_id, i.project_id, i.comment, i.created_at, p.email as author_email
            FROM inserted i JOIN profiles p ON i.user_id = p.id
            "#,
            project_id, user_id, text
        )
        .fetch_one(&self.pool).await.expect("Failed to add comment");

        // Manually map the anonymous record to the final enriched Comment struct.
        Comment { id: rec.id, user_id: rec.user_id, project_id: rec.project_id, comment: rec.comment, created_at: rec.created_at, author_email: Some(rec.author_email) }
    }

    /// get_comments
    ///
    /// Retrieves all comments for a project, enforcing the **Visibility Logic** by joining
    /// with the `projects` table and checking `pr.is_public = true`.
    async fn get_comments(&self, project_id: Uuid) -> Vec<Comment> {
        sqlx::query_as!(
            Comment,
            r#"
            SELECT 
                c.id, c.user_id, c.project_id, c.comment, c.created_at, p.email as author_email
            FROM project_comments c 
            JOIN profiles p ON c.user_id = p.id
            JOIN projects pr ON c.project_id = pr.id -- Enforces project existence/visibility
            WHERE c.project_id = $1 AND pr.is_public = true -- ADDED VISIBILITY CHECK
            ORDER BY c.created_at ASC
            "#,
            project_id
        ).fetch_all(&self.pool).await.unwrap_or_default()
    }

    /// delete_project_admin
    ///
    /// **Admin Override**: Deletes a project without checking ownership.
    async fn delete_project_admin(&self, id: Uuid) -> bool {
        match sqlx::query!("DELETE FROM projects WHERE id = $1", id).execute(&self.pool).await {
            Ok(res) => res.rows_affected() > 0,
            Err(e) => { tracing::error!("admin delete error: {:?}", e); false }
        }
    }

    /// delete_comment
    ///
    /// Deletes a comment only if the provided `user_id` matches the comment author.
    /// **Owner-Only** check.
    async fn delete_comment(&self, id: i64, user_id: Uuid) -> bool {
        match sqlx::query!("DELETE FROM project_comments WHERE id = $1 AND user_id = $2", id, user_id).execute(&self.pool).await {
            Ok(res) => res.rows_affected() > 0,
            Err(e) => { tracing::error!("delete comment error: {:?}", e); false }
        }
    }

    /// delete_comment_admin
    ///
    /// **Admin Override**: Deletes a comment without checking ownership.
    async fn delete_comment_admin(&self, id: i64) -> bool {
        match sqlx::query!("DELETE FROM project_comments WHERE id = $1", id).execute(&self.pool).await {
            Ok(res) => res.rows_affected() > 0,
            Err(e) => { tracing::error!("admin delete comment error: {:?}", e); false }
        }
    }

    // --- NOTIFICATIONS ---

    /// get_notifications
    ///
    /// Retrieves all notifications for a user, performing necessary JOINs to enrich the payload
    /// with the `actor_email` and `project_title` required by the `NotificationResponse` model.
    async fn get_notifications(&self, user_id: Uuid) -> Vec<crate::models::NotificationResponse> {
    let query = r#"
        SELECT 
            n.id, 
            u.email as actor_email, 
            n.project_id, 
            p.title as project_title, 
            n.type, 
            n.is_read, 
            n.created_at
        FROM notifications n
        JOIN profiles u ON n.actor_id = u.id -- Get the name/email of the liker/commenter
        JOIN projects p ON n.project_id = p.id -- Get the title of the project
        WHERE n.user_id = $1 -- Only for the recipient user
        ORDER BY n.created_at DESC
    "#;

    sqlx::query_as::<_, crate::models::NotificationResponse>(query)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to fetch notifications: {:?}", e);
            vec![]
        })
    }

    /// mark_notification_read
    ///
    /// Sets `is_read = true` for a notification, enforced by an **ownership check** (`user_id`).
    async fn mark_notification_read(&self, notification_id: Uuid, user_id: Uuid) -> bool {
    let result = sqlx::query("UPDATE notifications SET is_read = true WHERE id = $1 AND user_id = $2")
        .bind(notification_id)
        .bind(user_id)
        .execute(&self.pool)
        .await;

    match result {
        Ok(r) => r.rows_affected() > 0,
        Err(e) => {
            tracing::error!("Failed to mark notification read: {:?}", e);
            false
        }
    }
}
}
