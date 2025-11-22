use async_trait::async_trait;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use fyp_portal::{
    AppState,
    auth::AuthUser,
    config::AppConfig,
    handlers,
    models::{
        AdminDashboardStats, Comment, CreateProjectRequest, NotificationResponse,
        PresignedUrlRequest, Project, UpdateProjectRequest, User,
    },
    repository::Repository,
    storage::MockStorageService,
};
use std::sync::Arc;
use tokio::test;
use uuid::Uuid;

// --- MOCK REPOSITORY IMPLEMENTATION ---

// This struct is the central control point for testing handler logic.
// Handlers rely on traits, so we mock the trait implementation.
pub struct MockRepoControl {
    // Expected inputs to verify handler correctly extracts data
    pub project_creation_input: Option<(CreateProjectRequest, Uuid)>,
    pub delete_project_called: bool,
    pub delete_project_admin_called: bool,
    pub like_project_result: bool,
    pub get_project_result: Option<Project>,
    pub get_user_role: String,

    // Pre-canned outputs for handler requests
    pub projects_to_return: Vec<Project>,
    pub stats_to_return: AdminDashboardStats,
    pub notifications_to_return: Vec<NotificationResponse>,
}

impl Default for MockRepoControl {
    fn default() -> Self {
        MockRepoControl {
            project_creation_input: None,
            delete_project_called: false,
            delete_project_admin_called: false,
            like_project_result: true, // Default to success for simpler tests
            get_project_result: Some(Project::default()),
            get_user_role: "student".to_string(),
            projects_to_return: vec![],
            stats_to_return: AdminDashboardStats::default(),
            notifications_to_return: vec![],
        }
    }
}

#[async_trait]
impl Repository for MockRepoControl {
    // --- Handlers use these methods: ---
    async fn get_projects(&self, _year: Option<i32>, _search: Option<String>) -> Vec<Project> {
        self.projects_to_return.clone()
    }
    async fn get_all_projects(&self) -> Vec<Project> {
        self.projects_to_return.clone()
    }
    async fn get_top_projects(&self, _limit: i64) -> Vec<Project> {
        self.projects_to_return.clone()
    }
    async fn get_project(&self, _id: Uuid) -> Option<Project> {
        self.get_project_result.clone()
    }
    async fn get_stats(&self) -> AdminDashboardStats {
        self.stats_to_return.clone()
    }
    async fn get_my_projects(&self, _user_id: Uuid) -> Vec<Project> {
        self.projects_to_return.clone()
    }
    async fn get_notifications(&self, _user_id: Uuid) -> Vec<NotificationResponse> {
        self.notifications_to_return.clone()
    }

    // --- Verification Methods ---
    async fn create_project(&self, _req: CreateProjectRequest, _user_id: Uuid) -> Project {
        // In a real mock, you would record the input here
        Project::default() // Return a default struct to satisfy compiler
    }
    async fn like_project(&self, _like: fyp_portal::models::Like) -> bool {
        self.like_project_result
    }
    async fn delete_project(&self, _id: Uuid, _user_id: Uuid) -> bool {
        self.delete_project_called
    }
    async fn delete_project_admin(&self, _id: Uuid) -> bool {
        self.delete_project_admin_called
    }
    async fn update_project(
        &self,
        _id: Uuid,
        _user_id: Uuid,
        _req: UpdateProjectRequest,
    ) -> Option<Project> {
        self.get_project_result.clone()
    }
    async fn add_comment(&self, _project_id: Uuid, _user_id: Uuid, _text: String) -> Comment {
        Comment::default()
    }
    async fn get_comments(&self, _project_id: Uuid) -> Vec<Comment> {
        self.projects_to_return
            .clone()
            .into_iter()
            .map(|_| Comment::default())
            .collect()
    }
    async fn set_project_status(&self, _id: Uuid, _is_public: bool) -> Option<Project> {
        self.get_project_result.clone()
    }
    async fn mark_notification_read(&self, _notification_id: Uuid, _user_id: Uuid) -> bool {
        self.like_project_result
    }

    // Minimal mocks for compilation
    async fn get_user(&self, _id: Uuid) -> Option<User> {
        Some(User {
            id: _id,
            email: "test@user.com".to_string(),
            role: self.get_user_role.clone(),
        })
    }
    async fn create_user(&self, _user: User) -> User {
        User::default()
    }
    async fn delete_comment(&self, _id: i64, _user_id: Uuid) -> bool {
        self.delete_project_called
    }
    async fn delete_comment_admin(&self, _id: i64) -> bool {
        self.delete_project_admin_called
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

// --- TEST UTILITIES ---

const TEST_ID: Uuid = Uuid::from_u128(123);
const TEST_ADMIN_ID: Uuid = Uuid::from_u128(456);

// Creates an AppState using mock components
fn create_test_state(
    repo_control: MockRepoControl,
    storage_control: MockStorageService,
) -> AppState {
    AppState {
        repo: Arc::new(repo_control),
        storage: Arc::new(storage_control),
        config: AppConfig::default(),
    }
}

// Creates AuthUser for handler calls
fn admin_user() -> AuthUser {
    AuthUser {
        id: TEST_ADMIN_ID,
        role: "admin".to_string(),
    }
}
fn student_user() -> AuthUser {
    AuthUser {
        id: TEST_ID,
        role: "student".to_string(),
    }
}

// --- HANDLER TESTS ---

#[test]
async fn test_get_project_details_success() {
    let mock_project = Project::default();
    let state = create_test_state(
        MockRepoControl {
            get_project_result: Some(mock_project.clone()),
            ..MockRepoControl::default()
        },
        MockStorageService::new(),
    );

    let result = handlers::get_project_details(State(state), Path(TEST_ID)).await;

    assert!(result.is_ok());

    let response = result.unwrap();
    let axum_response = response.into_response();
    let (_parts, body) = axum_response.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let project: Project = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(project.id, mock_project.id);
}

#[test]
async fn test_get_project_details_not_found() {
    let state = create_test_state(
        MockRepoControl {
            get_project_result: None,
            ..MockRepoControl::default()
        },
        MockStorageService::new(),
    );

    let result = handlers::get_project_details(State(state), Path(TEST_ID)).await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), StatusCode::NOT_FOUND);
}

#[test]
async fn test_get_admin_projects_forbidden() {
    let state = create_test_state(MockRepoControl::default(), MockStorageService::new());

    // Call with a non-admin user
    let result = handlers::get_admin_projects(student_user(), State(state)).await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), StatusCode::FORBIDDEN);
}

#[test]
async fn test_get_admin_projects_success() {
    let mock_projects = vec![Project::default()];
    let state = create_test_state(
        MockRepoControl {
            projects_to_return: mock_projects.clone(),
            get_user_role: "admin".to_string(),
            ..MockRepoControl::default()
        },
        MockStorageService::new(),
    );

    // Call with admin user
    let result = handlers::get_admin_projects(admin_user(), State(state)).await;

    assert!(result.is_ok());
    let Json(projects) = result.unwrap();
    assert_eq!(projects.len(), 1);
}

#[test]
async fn test_vote_project_success() {
    let state = create_test_state(
        MockRepoControl {
            like_project_result: true,
            ..MockRepoControl::default()
        },
        MockStorageService::new(),
    );

    let result = handlers::vote_project(student_user(), State(state), Path(TEST_ID)).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), StatusCode::OK);
}

#[test]
async fn test_vote_project_conflict() {
    let state = create_test_state(
        MockRepoControl {
            like_project_result: false,
            ..MockRepoControl::default()
        },
        MockStorageService::new(),
    );

    let result = handlers::vote_project(student_user(), State(state), Path(TEST_ID)).await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), StatusCode::CONFLICT);
}

#[test]
async fn test_delete_project_not_found_or_not_owner() {
    let state = create_test_state(
        MockRepoControl {
            delete_project_called: false,
            ..MockRepoControl::default()
        },
        MockStorageService::new(),
    );

    let status = handlers::delete_project(student_user(), State(state), Path(TEST_ID)).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test]
async fn test_delete_project_success() {
    let state = create_test_state(
        MockRepoControl {
            delete_project_called: true,
            ..MockRepoControl::default()
        },
        MockStorageService::new(),
    );

    let status = handlers::delete_project(student_user(), State(state), Path(TEST_ID)).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[test]
async fn test_get_presigned_url_success() {
    // We remove the conflicting hardcoded upload_url_to_return line.

    let resource_key_prefix = "uploads/";

    // 1. Setup Mock Storage (MockStorageService handles the URL construction)
    let mock_storage = fyp_portal::storage::MockStorageService {
        should_fail: false,
        // NOTE: If your MockStorageService doesn't store state,
        // we must check the format, not the exact string.
    };

    // ... (rest of setup) ...
    let state = create_test_state(MockRepoControl::default(), mock_storage);
    let auth_user = student_user();

    // 2. Define the Request Payload
    let payload = PresignedUrlRequest {
        filename: "my_report.pdf".to_string(),
        file_type: "application/pdf".to_string(),
    };

    // --- EXECUTION ---
    let response = handlers::get_presigned_url(auth_user, State(state), Json(payload)).await;
    let response = response.into_response();
    let (parts, body) = response.into_parts();
    let status = parts.status;
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let body_json: fyp_portal::models::PresignedUrlResponse =
        serde_json::from_slice(&bytes).expect("Failed to deserialize JSON response from handler");

    // --- ASSERTIONS ---
    assert_eq!(status, StatusCode::OK);

    // FIX 1: Assert the upload_url STARTS with the mock prefix
    assert!(
        body_json
            .upload_url
            .starts_with("http://localhost:9000/mock-bucket/"),
        "Upload URL should start with the MockStorageService's base URL."
    );

    // FIX 2: Assert the upload_url CONTAINS the generated resource key (which proves the handler used the mock output)
    assert!(
        body_json.upload_url.contains(&body_json.resource_key),
        "Upload URL should contain the resource key generated by the handler."
    );

    // Assert resource key format (already mostly correct)
    assert!(body_json.resource_key.starts_with(resource_key_prefix));
    assert!(body_json.resource_key.ends_with(".pdf"));
}

#[test]
async fn test_mark_notification_read_success() {
    let state = create_test_state(
        MockRepoControl {
            like_project_result: true,
            ..MockRepoControl::default()
        },
        MockStorageService::new(),
    );

    let status =
        handlers::mark_notification_read(student_user(), State(state), Path(TEST_ID)).await;

    assert_eq!(status, StatusCode::OK);
}

#[test]
async fn test_delete_comment_admin_override() {
    let state = create_test_state(
        MockRepoControl {
            delete_project_admin_called: true,
            delete_project_called: false,
            ..MockRepoControl::default()
        },
        MockStorageService::new(),
    );

    // Call with an admin user
    let status = handlers::delete_comment(admin_user(), State(state), Path(123i64)).await;

    // Assert the handler took the admin path
    assert_eq!(status, StatusCode::NO_CONTENT);
}
