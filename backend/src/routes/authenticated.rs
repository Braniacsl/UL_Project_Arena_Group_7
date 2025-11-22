use crate::{AppState, handlers};
use axum::{
    Router,
    routing::{delete, get, post, put},
};

/// Authenticated Router Module
///
/// Defines the routes accessible to any user who has successfully passed the authentication layer.
/// This module implements all core application features for a standard user ('student' role),
/// including project submission, voting, commenting, and media upload.
///
/// Access Control Strategy:
/// Every handler in this module relies on the `AuthUser` extractor middleware being present
/// on the router layer above this module. This guarantees that all handlers receive a
/// validated `AuthUser` struct containing the user's ID and role, which is then used
/// for all Owner-Only authorization checks (e.g., in `update_project` and `delete_project`).
pub fn authenticated_routes() -> Router<AppState> {
    Router::<AppState>::new()
        // POST /upload/presigned
        // Initiates the secure media upload pipeline. Generates a short-lived (10-minute)
        // presigned S3 URL which allows the client to upload video/image/PDF content
        // directly to the storage service (S3/MinIO), bypassing the application server.
        .route("/upload/presigned", post(handlers::get_presigned_url))
        // GET /me
        // Retrieves the currently authenticated user's profile and session data.
        .route("/me", get(handlers::get_me))
        // GET /me/projects
        // Lists all projects owned by the authenticated user, including those that are
        // not yet public (`is_public=false`).
        .route("/me/projects", get(handlers::get_my_projects))
        // --- Project Submission & Voting ---
        // POST /projects
        // Submits a new project to the system. Requires `user_id` validation.
        .route("/projects", post(handlers::create_project))
        // PUT/DELETE /projects/{id}
        // Allows the user to modify or remove their own submitted project.
        // Strict ownership check** is enforced within the handler logic.
        .route(
            "/projects/{id}",
            put(handlers::update_project).delete(handlers::delete_project),
        )
        // POST /projects/{id}/vote
        // Registers a 'like' for a specific project. The handler implements **idempotency** // using the composite primary key on the `project_likes` table to prevent double voting.
        .route("/projects/{id}/vote", post(handlers::vote_project))
        // --- Commenting System ---
        // POST /projects/{id}/comments
        // Posts a new comment on a specified project.
        // This action triggers the PostgreSQL notification trigger (`handle_new_comment`).
        .route("/projects/{id}/comments", post(handlers::add_comment))
        // DELETE /comments/{id}
        // Allows a user to delete their own comment. Ownership validation is required.
        .route("/comments/{id}", delete(handlers::delete_comment))
        // --- Notification System ---
        // GET /notifications
        // Retrieves all pending and past notifications for the authenticated user (the recipient).
        // The query must join with `auth.users` to include the `actor_email`.
        .route("/notifications", get(handlers::get_notifications))
        // PATCH /notifications/{id}/read
        // Marks a specific notification as processed (`is_read=true`). Uses PATCH for partial update.
        .route(
            "/notifications/{id}/read",
            axum::routing::patch(handlers::mark_notification_read),
        )
}

