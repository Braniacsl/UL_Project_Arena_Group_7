use crate::{AppState, handlers};
use axum::{
    Router,
    routing::{get, put},
};

/// Admin Router Module
///
/// Defines the routes exclusively accessible to users with the 'admin' role.
/// These endpoints provide moderation, oversight, and statistical access for project management.
///
/// Access Control:
/// This entire router must be wrapped in a middleware layer that first authenticates
/// the user (using the `AuthUser` extractor) and then explicitly checks for the
/// `role='admin'` permission before allowing the request to proceed to the handler.
/// This prevents any unauthorized access to critical moderation functions.
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        // GET /admin/stats
        // Retrieves core dashboard metrics (e.g., Total Users, Projects, Likes, Pending Reviews).
        // Essential for system health monitoring and oversight.
        .route("/stats", get(handlers::get_admin_stats))
        // GET /admin/projects
        // Lists ALL projects in the system, including those marked as `is_public=false`
        // (hidden/pending review). Used for administrative review and queue management.
        .route("/projects", get(handlers::get_admin_projects))
        // PUT /projects/{id}/status
        // Allows an administrator to change a project's visibility (`is_public` field).
        // This is the core moderation endpoint used to Publish or Hide projects.
        //
        // Note: The visibility status route is often exposed at a project endpoint
        // but is protected by the admin role check in the handler.
        .route(
            "/projects/{id}/status",
            put(handlers::update_project_status),
        )

    // Missing Routes (See API Contract):
    // The router should also include routes for force-deleting projects and comments,
    // which are defined in the API Contract:
    // - DELETE /projects/:id (Force delete any project)
    // - DELETE /comments/:id (Force delete any comment)
    // These handlers would need to be added here for feature completeness.
}
