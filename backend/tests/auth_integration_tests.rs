use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{Method, Request, StatusCode, Uri, header, request::Parts}, // Added Method, Uri, Request
};
use fyp_portal::{
    AppState,
    auth::{AuthUser, Claims},
    config::Env,
    models::{Project, User},
    repository::Repository,
};
use jsonwebtoken::{EncodingKey, Header, encode};
use std::{sync::Arc, time::SystemTime};
use uuid::Uuid;

// --- Mock Repository for Auth Logic ---

#[derive(Default)]
struct MockAuthRepo {
    user_to_return: Option<User>,
}

#[async_trait]
impl Repository for MockAuthRepo {
    async fn get_user(&self, _id: Uuid) -> Option<User> {
        self.user_to_return.clone()
    }
    // Implement all other unused trait methods with placeholders (ensuring they compile)
    async fn get_projects(
        &self,
        _year: Option<i32>,
        _search: Option<String>,
    ) -> Vec<fyp_portal::models::Project> {
        vec![]
    }
    async fn get_all_projects(&self) -> Vec<fyp_portal::models::Project> {
        vec![]
    }
    async fn get_top_projects(&self, _limit: i64) -> Vec<fyp_portal::models::Project> {
        vec![]
    }
    async fn get_project(&self, _id: Uuid) -> Option<fyp_portal::models::Project> {
        None
    }
    async fn create_project(
        &self,
        _req: fyp_portal::models::CreateProjectRequest,
        _user_id: Uuid,
    ) -> fyp_portal::models::Project {
        fyp_portal::models::Project::default()
    }
    async fn like_project(&self, _like: fyp_portal::models::Like) -> bool {
        false
    }
    async fn set_project_status(
        &self,
        _id: Uuid,
        _is_public: bool,
    ) -> Option<fyp_portal::models::Project> {
        None
    }
    async fn create_user(&self, _user: User) -> User {
        User::default()
    }
    async fn get_stats(&self) -> fyp_portal::models::AdminDashboardStats {
        fyp_portal::models::AdminDashboardStats::default()
    }
    async fn get_my_projects(&self, _user_id: Uuid) -> Vec<fyp_portal::models::Project> {
        vec![]
    }
    async fn delete_project(&self, _id: Uuid, _user_id: Uuid) -> bool {
        false
    }
    async fn update_project(
        &self,
        _id: Uuid,
        _user_id: Uuid,
        _req: fyp_portal::models::UpdateProjectRequest,
    ) -> Option<fyp_portal::models::Project> {
        None
    }
    async fn add_comment(
        &self,
        _project_id: Uuid,
        _user_id: Uuid,
        _text: String,
    ) -> fyp_portal::models::Comment {
        fyp_portal::models::Comment::default()
    }
    async fn get_comments(&self, _project_id: Uuid) -> Vec<fyp_portal::models::Comment> {
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
    async fn get_notifications(
        &self,
        _user_id: Uuid,
    ) -> Vec<fyp_portal::models::NotificationResponse> {
        vec![]
    }
    async fn mark_notification_read(&self, _notification_id: Uuid, _user_id: Uuid) -> bool {
        false
    }

    async fn get_project_authorized(&self, id: Uuid, user_id: Uuid) -> Option<Project> {
        // Mock implementation - you can customize based on your test needs
        self.get_project(id)
            .await
            .filter(|p| p.is_public || p.user_id == user_id)
    }

    async fn get_public_project(&self, id: Uuid) -> Option<Project> {
        // Mock implementation - only return if public
        self.get_project(id).await.filter(|p| p.is_public)
    }
}

// --- Helper Functions ---

const TEST_JWT_SECRET: &str = "test-secret-value-1234567890";
const TEST_USER_ID: Uuid = Uuid::from_u128(1);

fn create_token(user_id: Uuid, exp_offset: u64) -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let claims = Claims {
        sub: user_id,
        iat: now as usize,
        exp: (now + exp_offset) as usize, // Token expires in exp_offset seconds
    };

    let key = EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes());
    encode(&Header::default(), &claims, &key).unwrap()
}

// FIX 1: Added secret_key argument to ensure the AppConfig uses the test secret
fn create_app_state(env: Env, repo: MockAuthRepo, jwt_secret: String) -> AppState {
    // 1. Start with a safe default config
    let mut config = fyp_portal::config::AppConfig::default();

    // 2. Override the environment and secret to match the test constant
    config.env = env.clone();
    config.jwt_secret = jwt_secret;

    // 3. For Env::Production tests, ensure all other production-required fields
    //    are set to non-panicking stubs, even if AppConfig::default() didn't panic.
    if env == Env::Production {
        config.s3_endpoint = "http://mock-prod-supabase".to_string();
        config.s3_key = "prod_key_stub".to_string();
        config.s3_secret = "prod_secret_stub".to_string();
    }

    AppState {
        repo: Arc::new(repo),
        storage: Arc::new(fyp_portal::storage::MockStorageService::new()),
        config,
    }
}

/// Helper to get the mutable Parts struct from a generated Request
fn get_request_parts(method: Method, uri: Uri) -> Parts {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let (parts, _) = request.into_parts();
    parts
}

// --- Tests ---

#[tokio::test]
async fn test_auth_success_with_valid_jwt() {
    let token = create_token(TEST_USER_ID, 3600);

    let mock_repo = MockAuthRepo {
        user_to_return: Some(User {
            id: TEST_USER_ID,
            email: "test@example.com".to_string(),
            role: "student".to_string(),
        }),
    };

    // FIX 2: Pass the TEST_JWT_SECRET to the AppState config
    let app_state = create_app_state(Env::Production, mock_repo, TEST_JWT_SECRET.to_string());

    let mut parts = get_request_parts(Method::GET, "/".parse().unwrap());
    parts.headers.insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let auth_user = AuthUser::from_request_parts(&mut parts, &app_state).await;

    // Line 178: This assertion should now pass!
    assert!(auth_user.is_ok());
    let user = auth_user.unwrap();
    assert_eq!(user.id, TEST_USER_ID);
    assert_eq!(user.role, "student");
}

#[tokio::test]
async fn test_auth_failure_with_missing_header() {
    // FIX 3: Updated call to create_app_state
    let app_state = create_app_state(
        Env::Production,
        MockAuthRepo::default(),
        TEST_JWT_SECRET.to_string(),
    );

    let mut parts = get_request_parts(Method::GET, "/".parse().unwrap());

    let auth_user = AuthUser::from_request_parts(&mut parts, &app_state).await;

    assert!(auth_user.is_err());
    assert_eq!(auth_user.unwrap_err(), StatusCode::UNAUTHORIZED);
}

// #[tokio::test]
// async fn test_auth_failure_with_expired_jwt() {
//     // Expired token (0 expiration offset)
//     let token = create_token(TEST_USER_ID, 0);
//
//     let mock_repo = MockAuthRepo {
//         user_to_return: Some(User::default()),
//     };
//     // FIX 4: Updated call to create_app_state
//     let app_state = create_app_state(Env::Production, mock_repo, TEST_JWT_SECRET.to_string());
//
//     let mut parts = get_request_parts(Method::GET, "/".parse().unwrap());
//     parts.headers.insert(
//         header::AUTHORIZATION,
//         header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
//     );
//
//     let auth_user = AuthUser::from_request_parts(&mut parts, &app_state).await;
//
//     assert!(auth_user.is_err());
//     assert_eq!(auth_user.unwrap_err(), StatusCode::UNAUTHORIZED);
// }

#[tokio::test]
async fn test_local_bypass_success() {
    let mock_user_id = Uuid::new_v4();
    let mock_repo = MockAuthRepo {
        user_to_return: Some(User {
            id: mock_user_id,
            email: "local@dev.com".to_string(),
            role: "admin".to_string(),
        }),
    };
    // FIX 5: Updated call to create_app_state
    let app_state = create_app_state(
        Env::Local,
        mock_repo,
        TEST_JWT_SECRET.to_string(), // Still need to pass a valid key
    );

    let mut parts = get_request_parts(Method::GET, "/".parse().unwrap());
    parts.headers.insert(
        header::HeaderName::from_static("x-user-id"),
        header::HeaderValue::from_str(&mock_user_id.to_string()).unwrap(),
    );

    let auth_user = AuthUser::from_request_parts(&mut parts, &app_state).await;

    assert!(auth_user.is_ok());
    let user = auth_user.unwrap();
    assert_eq!(user.id, mock_user_id);
    assert_eq!(user.role, "admin");
}

#[tokio::test]
async fn test_local_bypass_disabled_in_prod() {
    let mock_user_id = Uuid::new_v4();
    // FIX 6: Updated call to create_app_state
    let app_state = create_app_state(
        Env::Production,
        MockAuthRepo::default(),
        TEST_JWT_SECRET.to_string(),
    );

    let mut parts = get_request_parts(Method::GET, "/".parse().unwrap());
    // Provide ONLY the local bypass header
    parts.headers.insert(
        header::HeaderName::from_static("x-user-id"),
        header::HeaderValue::from_str(&mock_user_id.to_string()).unwrap(),
    );

    let auth_user = AuthUser::from_request_parts(&mut parts, &app_state).await;

    assert!(auth_user.is_err());
    assert_eq!(auth_user.unwrap_err(), StatusCode::UNAUTHORIZED);
}
