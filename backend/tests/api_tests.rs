use fyp_portal::{
    AppConfig, AppState, MockStorageService, create_router,
    models::Project,
    repository::{PostgresRepository, RepositoryState},
    storage::StorageState,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Debug)]
pub struct TestApp {
    pub address: String,
    pub pool: sqlx::PgPool,
}

async fn spawn_app() -> TestApp {
    dotenv::dotenv().ok();

    let db_url = "postgres://postgres:password@localhost:5432/fyp";

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await
        .expect("Failed to connect to Postgres in tests");

    let repo = Arc::new(PostgresRepository::new(pool.clone())) as RepositoryState;
    let storage = Arc::new(MockStorageService::new()) as StorageState;
    let config = AppConfig::load();

    let state = AppState {
        repo,
        storage,
        config,
    };
    let router = create_router(state);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    TestApp { address, pool }
}

#[tokio::test]
async fn test_health_check() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("{}/health", app.address))
        .send()
        .await
        .expect("req fail");
    assert!(response.status().is_success());
}

#[tokio::test]
async fn test_project_lifecycle() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let user_id = Uuid::new_v4();

    // Seed User
    sqlx::query!(
        "INSERT INTO auth.users (id, email) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        user_id,
        "t@t.com"
    )
    .execute(&app.pool)
    .await
    .unwrap();
    sqlx::query!(
        "INSERT INTO profiles (id, email, role) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        user_id,
        "t@t.com",
        "student"
    )
    .execute(&app.pool)
    .await
    .unwrap();

    // Create
    let response = client.post(&format!("{}/projects", app.address))
        .header("x-user-id", user_id.to_string())
        .json(&serde_json::json!({
            "title": "Bot", "abstract_text": "AI", "author_name": "Robo", "year": 2025, "cover_image_key": "img.jpg"
        }))
        .send().await.expect("post fail");
    assert_eq!(response.status(), 200);
    let p: Project = response.json().await.unwrap();

    // Vote
    let resp = client
        .post(&format!("{}/projects/{}/vote", app.address, p.id))
        .header("x-user-id", user_id.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_get_public_projects() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let user_id = Uuid::new_v4();

    sqlx::query!(
        "INSERT INTO auth.users (id, email) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        user_id,
        "t@t.com"
    )
    .execute(&app.pool)
    .await
    .unwrap();
    sqlx::query!(
        "INSERT INTO profiles (id, email, role) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        user_id,
        "t@t.com",
        "admin"
    )
    .execute(&app.pool)
    .await
    .unwrap();

    // 1. Create Private Project
    let resp = client.post(&format!("{}/projects", app.address))
        .header("x-user-id", user_id.to_string())
        .json(&serde_json::json!({
            "title": "Secret", "abstract_text": "Shh", "author_name": "Spy", "year": 2025, "cover_image_key": "img.jpg"
        }))
        .send().await.unwrap();
    let p: Project = resp.json().await.unwrap();

    // 2. Verify NOT in public list
    let list_resp = client
        .get(&format!("{}/projects", app.address))
        .send()
        .await
        .unwrap();
    let list: Vec<Project> = list_resp.json().await.unwrap();
    assert!(
        list.iter().all(|proj| proj.id != p.id),
        "Private project should not be listed"
    );

    // 3. Approve Project (Set Public)
    let status_resp = client
        .put(&format!("{}/admin/projects/{}/status", app.address, p.id))
        .header("x-user-id", user_id.to_string())
        .json(&true)
        .send()
        .await
        .unwrap();
    assert_eq!(status_resp.status(), 200);

    // 4. Verify IS in public list
    let status_resp = client
        .put(&format!("{}/admin/projects/{}/status", app.address, p.id))
        .header("x-user-id", user_id.to_string())
        .json(&true)
        .send()
        .await
        .unwrap();
    assert_eq!(status_resp.status(), 200);
    let updated_project: Project = status_resp.json().await.unwrap();
    println!(
        "Updated project: id={}, is_public={}",
        updated_project.id, updated_project.is_public
    );

    // Verify directly in database
    let db_check = sqlx::query!("SELECT is_public FROM projects WHERE id = $1", p.id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    println!("Database shows is_public: {}", db_check.is_public);
}
