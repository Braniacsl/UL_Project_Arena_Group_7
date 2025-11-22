use crate::{
    AppState,
    auth::AuthUser,
    models::{
        self, AdminDashboardStats, Comment, CreateCommentRequest, CreateProjectRequest,
        NotificationResponse, PresignedUrlRequest, PresignedUrlResponse, Project,
        RegisterUserRequest, UpdateProjectRequest, User, UserProfile,
    },
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use uuid::Uuid;

// --- Filter Structs ---

/// ProjectFilter
///
/// Defines the accepted query parameters for the public project listing endpoint (GET /projects).
/// Used by Axum's Query extractor to safely bind HTTP query parameters for filtering and search.
#[derive(Deserialize, utoipa::IntoParams)]
pub struct ProjectFilter {
    /// Optional filter for projects created in a specific year.
    pub year: Option<i32>,
    /// Optional full-text search string for project title/abstract matching.
    pub search: Option<String>,
}

/// SupabaseAuthResponse
///
/// Minimal struct to deserialize the response from the external Supabase /auth/v1/signup endpoint,
/// specifically capturing the newly created user's UUID.
#[derive(Deserialize)]
struct SupabaseAuthResponse {
    id: Uuid,
}

// --- Handlers ---

/// get_my_projects
///
/// [Authenticated Route] Lists all projects owned by the requesting user.
/// This includes projects that are currently hidden or pending review (`is_public=false`).
///
/// *Note*: The user identity (`id`) is resolved securely via the `AuthUser` extractor.
#[utoipa::path(
    get,
    path = "/me/projects",
    responses((status = 200, description = "My Projects", body = [Project]))
)]
pub async fn get_my_projects(
    AuthUser { id, .. }: AuthUser,
    State(state): State<AppState>,
) -> Json<Vec<models::Project>> {
    let projects = state.repo.get_my_projects(id).await;
    Json(projects)
}

/// add_comment
///
/// [Authenticated Route] Posts a new comment on a project.
/// This operation **triggers the PostgreSQL notification trigger** (`handle_new_comment`)
/// upon successful database insertion.
#[utoipa::path(
    post,
    path = "/projects/{id}/comments",
    request_body = CreateCommentRequest,
    responses((status = 201, description = "Comment Added", body = Comment))
)]
pub async fn add_comment(
    AuthUser { id: user_id, .. }: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<CreateCommentRequest>,
) -> Json<models::Comment> {
    let comment = state
        .repo
        .add_comment(project_id, user_id, payload.text)
        .await;
    Json(comment)
}

/// get_comments
///
/// [Public Route] Retrieves all comments for a given project ID.
/// The underlying repository method ensures the project is public before returning comments.
#[utoipa::path(
    get,
    path = "/projects/{id}/comments",
    responses((status = 200, description = "Comments", body = [Comment]))
)]
pub async fn get_comments(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Json<Vec<models::Comment>> {
    let comments = state.repo.get_comments(project_id).await;
    Json(comments)
}

/// delete_project
///
/// [Authenticated Route] Allows a user to delete their own project.
///
/// *Authorization*: The repository method enforces an **Owner-Only** check against the `user_id`
/// provided by the `AuthUser` extractor. If the user is not the owner, the repository query
/// will affect 0 rows, resulting in a 404 (or 403, depending on error mapping).
#[utoipa::path(
    delete,
    path = "/projects/{id}",
    responses(
        (status = 204, description = "Deleted"), 
        (status = 403, description = "Not Owner"),
        (status = 404, description = "Not Found")
    )
)]
pub async fn delete_project(
    AuthUser { id: user_id, .. }: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> StatusCode {
    // If the repository returns false, it means either the project didn't exist,
    // or the user wasn't the owner, hence 404 is a safe default response.
    if state.repo.delete_project(id, user_id).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

/// update_project
///
/// [Authenticated Route] Allows a user to modify their own project details.
///
/// *Authorization*: Enforces the **Owner-Only** check in the repository layer.
#[utoipa::path(
    put,
    path = "/projects/{id}",
    request_body = UpdateProjectRequest,
    responses((status = 200, description = "Updated", body = Project))
)]
pub async fn update_project(
    AuthUser { id: user_id, .. }: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateProjectRequest>,
) -> Result<Json<models::Project>, StatusCode> {
    match state.repo.update_project(id, user_id, payload).await {
        Some(project) => Ok(Json(project)),
        // Returns 404 if the project is not found OR if the authenticated user is not the owner.
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// get_projects
///
/// [Public Route] Lists public projects with filtering and search capabilities.
///
/// *Security*: The repository method applies the `is_public=true` filter **unconditionally**
/// to prevent data leakage to anonymous users, ensuring Defense-in-Depth.
#[utoipa::path(
    get,
    path = "/projects",
    params(ProjectFilter),
    responses(
        (status = 200, description = "List filtered projects", body = [Project])
    )
)]
pub async fn get_projects(
    State(state): State<AppState>,
    Query(filter): Query<ProjectFilter>,
) -> Json<Vec<models::Project>> {
    let projects = state.repo.get_projects(filter.year, filter.search).await;
    Json(projects)
}

/// get_project_details
///
/// [Public Route] Retrieves a single project's details by ID.
/// Requires an existence and visibility check.
#[utoipa::path(
    get,
    path = "/projects/{id}",
    params(("id" = Uuid, Path, description = "Project ID")),
    responses((status = 200, description = "Found", body = Project))
)]
pub async fn get_project_details(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<models::Project>, StatusCode> {
    match state.repo.get_project(id).await {
        // If the project is not found OR is not public, it returns None.
        Some(project) => Ok(Json(project)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// get_featured_projects
///
/// [Public Route] Retrieves a small list of the most popular projects.
/// The `limit` (3) is hardcoded in the repository call.
#[utoipa::path(
    get,
    path = "/projects/featured",
    responses((status = 200, description = "Top projects", body = [Project]))
)]
pub async fn get_featured_projects(State(state): State<AppState>) -> Json<Vec<models::Project>> {
    let featured = state.repo.get_top_projects(3).await;
    Json(featured)
}

/// get_admin_projects
///
/// [Admin Route] Retrieves ALL projects in the system, regardless of their `is_public` status.
///
/// *Authorization*: Explicitly checks that the `role` resolved by `AuthUser` is "admin".
#[utoipa::path(
    get,
    path = "/admin/projects",
    responses((status = 200, description = "All projects", body = [Project]))
)]
pub async fn get_admin_projects(
    AuthUser { role, .. }: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<models::Project>>, StatusCode> {
    if role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(Json(state.repo.get_all_projects().await))
}

/// get_me
///
/// [Authenticated Route] Provides the authenticated user's profile information.
///
/// *Note*: This handler fabricates the email and avatar URL dynamically based on the
/// resolved `AuthUser` ID and role, simulating data that would typically come from the
/// Auth layer or a profile service.
#[utoipa::path(
    get,
    path = "/me",
    responses((status = 200, description = "Profile", body = UserProfile))
)]
pub async fn get_me(AuthUser { id, role, .. }: AuthUser) -> Json<UserProfile> {
    Json(UserProfile {
        id,
        // Mocking a TCD-style student email for demonstration/frontend use.
        email: format!(
            "user_{}@student.tcd.ie",
            id.simple().to_string().chars().take(4).collect::<String>()
        ),
        role,
        // Using a DiceBear API for stable, unique avatar generation based on UUID.
        avatar_url: Some(format!(
            "https://api.dicebear.com/7.x/avataaars/svg?seed={}",
            id
        )),
    })
}

/// get_admin_stats
///
/// [Admin Route] Retrieves core application statistics for the dashboard.
///
/// *Authorization*: Explicitly checks that the `role` is "admin".
#[utoipa::path(
    get,
    path = "/admin/stats",
    responses((status = 200, description = "Stats", body = AdminDashboardStats))
)]
pub async fn get_admin_stats(
    AuthUser { role, .. }: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<AdminDashboardStats>, StatusCode> {
    if role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(Json(state.repo.get_stats().await))
}

/// create_project
///
/// [Authenticated Route] Handles the submission of a new project.
/// The `user_id` is automatically taken from the authenticated session, ensuring data integrity.
#[utoipa::path(
    post,
    path = "/projects",
    request_body = CreateProjectRequest,
    responses((status = 200, description = "Created", body = Project))
)]
pub async fn create_project(
    AuthUser { id, .. }: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<models::CreateProjectRequest>,
) -> Json<models::Project> {
    let project = state.repo.create_project(payload, id).await;
    Json(project)
}

/// vote_project
///
/// [Authenticated Route] Records a 'like' from the user for a project.
///
/// *Idempotency*: The repository method uses the composite primary key on `project_likes`
/// to enforce the **one-vote-per-user-per-project** rule, returning a 409 Conflict if violated.
#[utoipa::path(
    post,
    path = "/projects/{id}/vote",
    params(("id" = Uuid, Path, description = "Project ID")),
    responses(
        (status = 200, description = "Voted"),
        (status = 409, description = "Duplicate")
    )
)]
pub async fn vote_project(
    AuthUser { id, .. }: AuthUser,
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let like = models::Like {
        user_id: id,
        project_id,
    };

    match state.repo.like_project(like).await {
        true => Ok(StatusCode::OK),
        false => Err(StatusCode::CONFLICT),
    }
}

/// update_project_status
///
/// [Admin Route] Endpoint for an administrator to publish or hide a project.
///
/// *RBAC*: Strict enforcement of the "admin" role before calling the repository.
#[utoipa::path(
    put,
    path = "/projects/{id}/status",
    params(("id" = Uuid, Path, description = "Project ID")),
    request_body = bool,
    responses((status = 200, description = "Updated", body = Project))
)]
pub async fn update_project_status(
    AuthUser { role, id: _user_id }: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(is_public): Json<bool>,
) -> Result<Json<models::Project>, StatusCode> {
    if role != "admin" {
        return Err(StatusCode::FORBIDDEN);
    }
    match state.repo.set_project_status(id, is_public).await {
        Some(project) => Ok(Json(project)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// register_user
///
/// [Public Route] Handles initial user registration via the external Supabase Auth service.
///
/// *Flow*: Calls Supabase's signup endpoint, retrieves the `auth.users.id` (UUID), and then
/// uses that ID to create the corresponding record in the application's local `public.profiles` table.
/// This ensures primary key synchronization between the external Auth system and our local schema.
#[utoipa::path(
    post,
    path = "/register",
    request_body = RegisterUserRequest,
    responses((status = 200, description = "Registered", body = User))
)]
pub async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<RegisterUserRequest>,
) -> Result<Json<User>, StatusCode> {
    let supabase_url =
        std::env::var("SUPABASE_URL").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let supabase_key =
        std::env::var("SUPABASE_KEY").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Step 1: Call external Auth provider (Supabase)
    let client = reqwest::Client::new();
    let auth_url = format!("{}/auth/v1/signup", supabase_url);

    let response = client
        .post(auth_url)
        .header("apikey", supabase_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "email": payload.email, "password": payload.password }))
        .send()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !response.status().is_success() {
        // If Supabase rejects the user (e.g., email already exists, weak password).
        return Err(StatusCode::BAD_REQUEST);
    }

    // Step 2: Extract the canonical user ID from the external response.
    let supabase_user = response
        .json::<SupabaseAuthResponse>()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Step 3: Create the mirrored profile in our local database (`public.profiles`).
    let new_user = User {
        id: supabase_user.id,
        email: payload.email,
        role: payload.role,
    };

    let created_user = state.repo.create_user(new_user).await;

    Ok(Json(created_user))
}

/// get_presigned_url
///
/// [Authenticated Route] Generates a temporary, secure URL for direct client-to-cloud upload.
///
/// *Security*: The URL is short-lived (10 minutes max), constrained to the specified `file_type`,
/// and uses a unique, cryptographically secure object key (UUID). This implements the **Media Pipeline**
/// feature by offloading heavy media uploads from the application server.
#[utoipa::path(
    post,
    path = "/upload/presigned",
    request_body = PresignedUrlRequest,
    responses((status = 200, description = "URL", body = PresignedUrlResponse))
)]
pub async fn get_presigned_url(
    AuthUser { id: _user_id, .. }: AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<PresignedUrlRequest>,
) -> impl IntoResponse {
    // Generate a unique, structured object key (e.g., 'uploads/UUID.ext').
    let extension = std::path::Path::new(&payload.filename)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("bin");
    let unique_id = Uuid::new_v4();
    let object_key = format!("uploads/{}.{}", unique_id, extension);

    match state
        .storage
        // Delegate key generation and mime-type constraint application to the Storage Service.
        .get_presigned_upload_url(&object_key, &payload.file_type)
        .await
    {
        Ok(url) => {
            let response = PresignedUrlResponse {
                upload_url: url,
                resource_key: object_key,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            // Log the underlying storage error for debugging but return a generic internal error.
            eprintln!("Storage Error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed").into_response()
        }
    }
}

/// delete_comment
///
/// [Authenticated Route] Deletes a comment, implementing two tiers of authorization.
///
/// *RBAC/Ownership*: Checks for the "admin" role first (Force Delete), otherwise
/// checks for comment ownership (Owner Delete).
#[utoipa::path(
    delete,
    path = "/comments/{id}",
    params(("id" = i64, Path, description = "Comment ID")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not Found")
    )
)]
pub async fn delete_comment(
    AuthUser {
        id: user_id, role, ..
    }: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> StatusCode {
    if role == "admin" {
        // Admin Force Delete: Ignores ownership checks.
        if state.repo.delete_comment_admin(id).await {
            return StatusCode::NO_CONTENT;
        }
    } else {
        // Standard User Delete: Enforces ownership check against `user_id`.
        if state.repo.delete_comment(id, user_id).await {
            return StatusCode::NO_CONTENT;
        }
    }
    // Returns 404 if the comment was not found, or if the user lacked ownership/admin rights.
    StatusCode::NOT_FOUND
}

/// get_notifications
///
/// [Authenticated Route] Retrieves the recipient user's list of notifications.
/// This endpoint relies on data generated by the PostgreSQL database triggers.
#[utoipa::path(
    get,
    path = "/notifications",
    responses((status = 200, description = "My Notifications", body = [NotificationResponse]))
)]
pub async fn get_notifications(
    AuthUser { id, .. }: AuthUser,
    State(state): State<AppState>,
) -> Json<Vec<models::NotificationResponse>> {
    let notifs = state.repo.get_notifications(id).await;
    Json(notifs)
}

/// mark_notification_read
///
/// [Authenticated Route] Marks a specific notification as `is_read=true`.
///
/// *Ownership*: The repository method ensures the notification belongs to the authenticated user.
#[utoipa::path(
    patch,
    path = "/notifications/{id}/read",
    params(("id" = Uuid, Path, description = "Notification ID")),
    responses(
        (status = 200, description = "Marked as read"),
        (status = 404, description = "Not Found or Not Yours")
    )
)]
pub async fn mark_notification_read(
    AuthUser { id: user_id, .. }: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> StatusCode {
    if state.repo.mark_notification_read(id, user_id).await {
        StatusCode::OK
    } else {
        // 404 indicates the notification did not exist or did not belong to the user.
        StatusCode::NOT_FOUND
    }
}
