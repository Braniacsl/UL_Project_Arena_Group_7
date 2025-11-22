use crate::{AppState, handlers};
use axum::{
    Router,
    routing::{get, post},
};

/// Public Router Module
///
/// Defines endpoints that are **unauthenticated** and accessible to any client
/// (anonymous or logged-in). These routes primarily handle read-only data access
/// that has been explicitly marked as public, and core gateway functions like registration.
///
/// Security Mandate:
/// All data retrieval handlers in this module (i.e., `/projects/*`) must enforce
/// `is_public=true` at the Repository level. This prevents anonymous or unauthorized
/// viewing of projects pending review or explicitly hidden by an admin.
pub fn public_routes() -> Router<AppState> {
    Router::new()
        // GET /health
        // A simple, unauthenticated endpoint used for monitoring and load balancer checks.
        // Returns "ok" immediately to verify the service is running and responsive.
        .route("/health", get(|| async { "ok" }))
        // POST /register
        // Endpoint for new user creation and initial profile setup. This is part of the
        // identity flow managed by Supabase/Auth in production.
        .route("/register", post(handlers::register_user))
        // GET /projects?year=...&search=...
        // Lists all public projects, supporting filtering by year and full-text search.
        // Critical enforcement of `is_public=true` occurs in the handler's Repository query.
        .route("/projects", get(handlers::get_projects))
        // GET /projects/featured
        // Retrieves the top 3 projects ranked by the current like count.
        .route("/projects/featured", get(handlers::get_featured_projects))
        // GET /projects/{id}
        // Retrieves the detailed view of a single project.
        // Requires a repository-level check to ensure `is_public=true` before data release.
        .route("/projects/{id}", get(handlers::get_project_details))
        // GET /projects/{id}/comments
        // Lists all associated comments for a specific project.
        // This endpoint implicitly verifies that the parent project is public before retrieving comments.
        .route("/projects/{id}/comments", get(handlers::get_comments))
}
