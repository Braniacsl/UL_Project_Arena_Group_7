use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use fyp_portal::{
    AppConfig, AppState, create_router,
    models::{
        AdminDashboardStats, Comment, CreateProjectRequest, Like, NotificationResponse,
        PresignedUrlRequest, PresignedUrlResponse, Project, UpdateProjectRequest, User,
    },
    repository::{Repository, RepositoryState},
    storage::MockStorageService,
};
use std::sync::Arc;
use tower::util::ServiceExt;
use uuid::Uuid;

struct StubRepository;

#[async_trait]
impl Repository for StubRepository {
    async fn get_projects(&self, _y: Option<i32>, _s: Option<String>) -> Vec<Project> {
        vec![]
    }
    async fn get_all_projects(&self) -> Vec<Project> {
        vec![]
    }
    async fn get_top_projects(&self, _l: i64) -> Vec<Project> {
        vec![]
    }
    async fn get_project(&self, _id: Uuid) -> Option<Project> {
        None
    }
    async fn create_project(&self, _r: CreateProjectRequest, _u: Uuid) -> Project {
        panic!("Stub called")
    }
    async fn like_project(&self, _l: Like) -> bool {
        false
    }
    async fn set_project_status(&self, _id: Uuid, _p: bool) -> Option<Project> {
        None
    }
    async fn get_user(&self, id: Uuid) -> Option<User> {
        // Return a valid test user for any UUID
        Some(User {
            id,
            email: "test@test.com".to_string(),
            role: "student".to_string(),
        })
    }
    async fn create_user(&self, _u: User) -> User {
        panic!("Stub called")
    }
    async fn get_stats(&self) -> AdminDashboardStats {
        AdminDashboardStats {
            total_projects: 0,
            total_users: 0,
            total_likes: 0,
            pending_reviews: 0,
        }
    }

    async fn get_my_projects(&self, _user_id: Uuid) -> Vec<Project> {
        vec![]
    }

    async fn delete_project(&self, _id: Uuid, _user_id: Uuid) -> bool {
        false
    }

    async fn update_project(
        &self,
        _id: Uuid,
        _user_id: Uuid,
        _req: UpdateProjectRequest,
    ) -> Option<Project> {
        None
    }

    async fn add_comment(&self, _p_id: Uuid, _u_id: Uuid, _text: String) -> Comment {
        panic!("Stub called")
    }

    async fn get_comments(&self, _project_id: Uuid) -> Vec<Comment> {
        vec![]
    }

    async fn delete_project_admin(&self, _id: Uuid) -> bool {
        false
    }
    async fn delete_comment(&self, _id: i64, _user_id: Uuid) -> bool {
        false
    }
    async fn delete_comment_admin(&self, _id: i64) -> bool {
        false
    }

    async fn get_notifications(&self, _user_id: Uuid) -> Vec<NotificationResponse> {
        vec![]
    }

    async fn mark_notification_read(&self, _notification_id: Uuid, _user_id: Uuid) -> bool {
        false
    }

    async fn get_project_authorized(&self, id: Uuid, user_id: Uuid) -> Option<Project> {
        self.get_project(id)
            .await
            .filter(|p| p.is_public || p.user_id == user_id)
    }

    async fn get_public_project(&self, id: Uuid) -> Option<Project> {
        self.get_project(id).await.filter(|p| p.is_public)
    }
}

//#[cfg(test)]
fn setup_test_environment() {
    // Attempt to load the .env file. Use 'once' to prevent error if called multiple times.
    // We use dotenvy's function to ensure it loads into the process's environment.
    // If using the simple 'dotenv' crate, use 'dotenv::dotenv().ok();'
    // Since we don't know which is used, let's just make the simple call, and ensure the crate is a dependency.
    dotenv::dotenv().ok();
}

fn app(mock_storage: MockStorageService) -> axum::Router {
    #[cfg(test)]
    setup_test_environment();

    let repo = Arc::new(StubRepository) as RepositoryState;
    let storage = Arc::new(mock_storage);
    let config = AppConfig::load();

    let state = AppState {
        repo,
        storage,
        config,
    };
    create_router(state)
}

#[tokio::test]
async fn test_presigned_url_success() {
    let app = app(MockStorageService::new());
    let user_id = Uuid::new_v4(); // Add this

    let payload = PresignedUrlRequest {
        filename: "test_video.mp4".to_string(),
        file_type: "video/mp4".to_string(),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload/presigned")
                .header("Content-Type", "application/json")
                .header("x-user-id", user_id.to_string()) // Add this line
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: PresignedUrlResponse = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json.upload_url.contains("signature=fake"));
    assert!(body_json.resource_key.ends_with(".mp4"));
    assert!(body_json.resource_key.starts_with("uploads/"));
}

#[tokio::test]
async fn test_presigned_url_sanitization() {
    let app = app(MockStorageService::new());
    let user_id = Uuid::new_v4(); // Add this

    let payload = PresignedUrlRequest {
        filename: "../../etc/passwd.exe".to_string(),
        file_type: "application/binary".to_string(),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload/presigned")
                .header("Content-Type", "application/json")
                .header("x-user-id", user_id.to_string()) // Add this line
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: PresignedUrlResponse = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json.resource_key.ends_with(".exe"));
    assert!(!body_json.resource_key.contains(".."));
}

#[tokio::test]
async fn test_presigned_url_storage_failure() {
    let app = app(MockStorageService::new_failing());
    let user_id = Uuid::new_v4(); // Add this

    let payload = PresignedUrlRequest {
        filename: "valid.mp4".to_string(),
        file_type: "video/mp4".to_string(),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/upload/presigned")
                .header("Content-Type", "application/json")
                .header("x-user-id", user_id.to_string()) // Add this line
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
