use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use ts_rs::TS;
use utoipa::ToSchema;
use uuid::Uuid;

// --- Core Application Schemas (Mapped to Database) ---

/// User
///
/// Represents the user's canonical identity record stored in the `public.profiles` table.
/// This structure includes the minimal required data resolved during authentication.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, FromRow, Default)]
#[ts(export)]
pub struct User {
    // Primary Key, also the Foreign Key to the external auth.users table.
    pub id: Uuid,
    // The user's primary identifier.
    pub email: String,
    // The RBAC field: 'student' or 'admin'.
    pub role: String,
}

/// Project
///
/// Represents a final year project record from the `public.projects` table.
/// This is the primary data structure for the core business logic.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, FromRow, Default)]
#[ts(export)]
pub struct Project {
    pub id: Uuid,
    // FK to public.profiles.id (Owner).
    pub user_id: Uuid,
    pub author: String,
    pub title: String,

    /// Maps SQL column "abstract" to Rust field "abstract_text".
    /// This renaming is necessary because `abstract` is a reserved keyword in Rust.
    #[sqlx(rename = "abstract")]
    pub abstract_text: String,

    // S3 Keys for media assets.
    pub cover_image: String,
    pub video: Option<String>,
    pub report: Option<String>,

    // Logic Fields
    // Controls public visibility (enforced at the Repository layer).
    pub is_public: bool,
    // Allows separate control over the report document visibility, even if the project is public.
    pub report_is_public: bool,
    pub year: i32,

    // Timestamp handling for database integration and JSON serialization.
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "string")]
    pub updated_at: DateTime<Utc>,
}

/// Like
///
/// Internal structure representing a single vote record in the `public.project_likes` table.
/// It is only used internally by the repository for insertion and validation checks.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, Default)]
#[ts(export)]
pub struct Like {
    // Composite PK component 1: The user who cast the vote.
    pub user_id: Uuid,
    // Composite PK component 2: The project that received the vote.
    pub project_id: Uuid,
}

/// --- Request Payloads (Input Schemas) ---

/// CreateProjectRequest
///
/// Input payload for submitting a new project (POST /projects).
/// The S3 keys are provided here after the client completes the direct-to-cloud upload.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, Default)]
#[ts(export)]
pub struct CreateProjectRequest {
    pub title: String,
    pub abstract_text: String,
    pub author_name: String,
    pub year: i32,
    // S3 Key resulting from the presigned upload flow.
    pub cover_image_key: String,
    pub video_key: Option<String>,
    pub report_key: Option<String>,
}

/// RegisterUserRequest
///
/// Input payload for the public registration endpoint (POST /register).
/// Note: The password is only passed through to the external Auth provider (Supabase) and never
/// persisted or logged internally by this application.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct RegisterUserRequest {
    pub email: String,
    pub password: String,
    pub role: String,
}

/// PresignedUrlRequest
///
/// Input payload for requesting a short-lived S3 upload URL (POST /upload/presigned).
/// The server uses these fields to set security constraints on the generated URL.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, TS, Default)]
#[ts(export)]
pub struct PresignedUrlRequest {
    /// The original filename, used to derive the file extension.
    #[schema(example = "demo_video.mp4")]
    pub filename: String,
    /// The MIME type, used to constrain the S3 upload to the allowed type (security).
    #[schema(example = "video/mp4")]
    pub file_type: String,
}

/// PresignedUrlResponse
///
/// Output schema containing the secure, temporary URL for client-to-cloud file transfer.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, TS, Default)]
#[ts(export)]
pub struct PresignedUrlResponse {
    /// The time-limited URL for the PUT request.
    pub upload_url: String,
    /// The S3 object key where the file will be stored (used to reference the file in the database).
    pub resource_key: String,
}

/// CreateCommentRequest
///
/// Input payload for posting a new comment.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, Default)]
#[ts(export)]
pub struct CreateCommentRequest {
    pub text: String,
}

/// UpdateProjectRequest
///
/// Partial update payload for modifying an existing project (PUT /projects/{id}).
///
/// *Optimization*: Uses `Option<T>` for all fields and `#[serde(skip_serializing_if = "Option::is_none")]`
/// to efficiently handle partial updates, ensuring only provided fields are included in the JSON payload.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, Default)]
#[ts(export)]
pub struct UpdateProjectRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstract_text: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_image_key: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_key: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_key: Option<String>,
}

/// --- Dashboard & Profile Schemas (Output) ---

/// AdminDashboardStats
///
/// Output schema for the administrative statistics dashboard (GET /admin/stats).
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, Default)]
#[ts(export)]
pub struct AdminDashboardStats {
    pub total_projects: i64,
    pub total_users: i64,
    pub total_likes: i64,
    /// The number of projects where `is_public` is false.
    pub pending_reviews: i64,
}

/// UserProfile
///
/// Output schema for the authenticated user's profile (GET /me).
/// Provides a slightly richer set of data than the internal `User` struct.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, Default)]
#[ts(export)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    // Dynamic URL for a profile image/avatar.
    pub avatar_url: Option<String>,
}

/// Comment
///
/// Represents a comment record from the `public.project_comments` table, augmented with
/// the author's email (a join operation).
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, FromRow, Default)]
#[ts(export)]
pub struct Comment {
    // Using BigInt (i64) for comment ID due to the high volume potential.
    pub id: i64,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub comment: String,
    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
    // This field is loaded via a JOIN in the repository query.
    #[sqlx(default)]
    pub author_email: Option<String>,
}

/// --- Notification System Schemas ---

/// Notification
///
/// Raw Database Row (Internal Use). Directly maps to the `public.notifications` table.
/// This structure is used internally by the Repository before being transformed into the `NotificationResponse`.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, Default)]
pub struct Notification {
    pub id: Uuid,
    // Recipient (Project Owner)
    pub user_id: Uuid,
    // Trigger (Liker/Commenter)
    pub actor_id: Uuid,
    pub project_id: Uuid,

    // 'type' is a reserved keyword in Rust, so we rename it for internal Rust use.
    #[sqlx(rename = "type")]
    pub notification_type: String,

    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}

/// NotificationResponse
///
/// Enriched response structure for the Frontend (UI Ready).
/// This is the result of joining the internal `Notification` row with user and project details.
#[derive(Debug, Clone, Serialize, Deserialize, TS, ToSchema, FromRow, Default)]
#[ts(export)]
pub struct NotificationResponse {
    pub id: Uuid,

    // Who triggered it? (e.g., "Alice")
    pub actor_email: String,

    // What project? (e.g., "Rust Backend")
    pub project_id: Uuid,
    pub project_title: String,

    // Type: "like" | "comment"
    // We send it as "type" in JSON for API compatibility but read it as `notification_type` in Rust.
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub notification_type: String,

    pub is_read: bool,

    #[ts(type = "string")]
    pub created_at: DateTime<Utc>,
}
