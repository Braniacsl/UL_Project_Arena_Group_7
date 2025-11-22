use axum::{
    extract::{FromRef, Request}, 
    http::HeaderName,
    Router,
    middleware::{self, Next},
    response::Response, 
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::{Level, Span};

// --- Module Structure ---

// Core application services and components.
pub mod auth;
pub mod handlers;
pub mod models;
pub mod repository;
pub mod storage;
pub mod config;

// Module for routing segregation (Public, Authenticated, Admin).
pub mod routes;
use routes::{public, authenticated, admin};
use auth::AuthUser; // The resolved authenticated user identity.

// --- Public Re-exports ---

// Makes core state types easily accessible to the main application entry point (main.rs).
pub use config::AppConfig;
pub use repository::{RepositoryState, PostgresRepository};
pub use storage::{MockStorageService, S3StorageClient, StorageState};

/// ApiDoc
///
/// This struct auto-generates the OpenAPI documentation (Swagger JSON) for the application.
/// It aggregates all API paths and data schemas that have been decorated with
/// the `#[utoipa::path]` and `#[derive(utoipa::ToSchema)]` macros.
/// The resulting JSON is served at `/api-docs/openapi.json`.
#[derive(OpenApi)]
#[openapi(
    // List all public handler functions here for documentation generation.
    paths(
        handlers::get_projects, handlers::get_project_details, handlers::get_featured_projects, 
        handlers::get_admin_projects, handlers::create_project, handlers::vote_project, 
        handlers::update_project_status, handlers::get_presigned_url, handlers::register_user, 
        handlers::get_me, handlers::get_admin_stats, handlers::get_my_projects, 
        handlers::add_comment, handlers::get_comments, handlers::delete_project, 
        handlers::update_project, handlers::delete_comment, handlers::get_notifications,
        handlers::mark_notification_read
    ),
    // List all models (schemas) used in the request/response bodies.
    components(
        schemas(
            models::Project, models::CreateProjectRequest, models::UpdateProjectRequest,
            models::Like, models::Comment, models::CreateCommentRequest, models::PresignedUrlRequest, 
            models::PresignedUrlResponse, models::AdminDashboardStats, models::UserProfile,
            models::NotificationResponse,
        )
    ),
    tags(
        (name = "fyp-showcase", description = "FYP Project Showcase API")
    )
)]
struct ApiDoc;

/// AppState
///
/// Implements the **Unified State Pattern**. This is the single, thread-safe, and immutable
/// container holding all essential application services and configuration.
/// The application state is shared across all incoming requests.
#[derive(Clone)]
pub struct AppState {
    /// Repository Layer: Abstracts database access via the PgPool connection.
    pub repo: RepositoryState,
    /// Storage Layer: Abstracts S3/MinIO access and presigned URL generation.
    pub storage: StorageState,
    /// Configuration: The loaded, immutable environment configuration.
    pub config: AppConfig,
}

// --- Axum FromRef Extractor Implementations ---

// These implementations allow handlers to selectively pull components from the shared AppState.
// This is critical for dependency injection and adhering to the Clean Architecture boundaries.

impl FromRef<AppState> for RepositoryState {
    fn from_ref(app_state: &AppState) -> RepositoryState {
        app_state.repo.clone()
    }
}

impl FromRef<AppState> for StorageState {
    fn from_ref(app_state: &AppState) -> StorageState {
        app_state.storage.clone()
    }
}

impl FromRef<AppState> for AppConfig {
    fn from_ref(app_state: &AppState) -> AppConfig {
        app_state.config.clone()
    }
}

/// auth_middleware
///
/// A middleware function that enforces authentication for the `authenticated_routes`.
///
/// *Mechanism*: It attempts to extract `AuthUser` from the request. Since `AuthUser`
/// implements `FromRequestParts`, if authentication (JWT validation, DB lookup) fails,
/// the extractor immediately rejects the request with a 401 Unauthorized status,
/// preventing execution of the handler. If successful, it allows the request to proceed.
async fn auth_middleware(
    _auth_user: AuthUser,
    request: Request,
    next: Next,
) -> Response {
    next.run(request).await
}

/// create_router
///
/// Assembles the application's entire routing structure, applies global and scoped middleware,
/// and registers the application state.
pub fn create_router(state: AppState) -> Router {
    // 1. CORS Configuration
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    // Header name constant for Request Correlation.
    let x_request_id = HeaderName::from_static("x-request-id");

    // 2. Base Router Assembly
    let base_router = Router::new()
        // Documentation: Serve the auto-generated Swagger UI.
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        
        // Public Routes: No middleware applied.
        .merge(public::public_routes())
        
        // Authenticated Routes: Protected by the `auth_middleware`.
        // This implements the first layer of Defense-in-Depth for these routes.
        .merge(
            authenticated::authenticated_routes()
                .route_layer(middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware
                ))
        )
        
        // Admin Routes: Nested under '/admin'. The 'admin' role check is performed
        // *inside* the handlers after the request passes the authentication layer above.
        .nest("/admin", admin::admin_routes())
        
        // Apply the Unified State to all routes.
        .with_state(state); 

    // 3. Observability and Correlation Layers (Applied outermost/first)
    // This section implements the Production Observability Stack.
    base_router
        .layer(
             ServiceBuilder::new()
                 // 3a. Request ID Generation: Generates a unique UUID for every incoming request.
                 .layer(SetRequestIdLayer::new(
                     x_request_id.clone(),
                     MakeRequestUuid,
                 ))
                 // 3b. Request Tracing: Wraps the entire request/response lifecycle in a tracing span.
                 // Uses the `trace_span_logger` to include the generated request ID.
                 .layer(
                     TraceLayer::new_for_http()
                         .make_span_with(trace_span_logger)
                         .on_response(
                             DefaultOnResponse::new()
                                 .level(Level::INFO)
                                 .latency_unit(tower_http::LatencyUnit::Millis)
                         )
                 )
                 // 3c. Request ID Propagation: Ensures the generated x-request-id header is
                 // returned to the client and injected into subsequent service calls.
                 .layer(PropagateRequestIdLayer::new(x_request_id))
        )
        // 4. CORS Layer (Applied last, allowing all traffic in/out after processing)
        .layer(cors)
}

/// trace_span_logger
///
/// Helper function used by `TraceLayer` to customize the tracing span creation.
/// It extracts the `x-request-id` header (if present) and includes it in the
/// structured logging metadata alongside the HTTP method and URI.
///
/// *Goal*: Ensure every log line for a single request is correlated by a unique ID.
fn trace_span_logger(request: &axum::http::Request<axum::body::Body>) -> Span {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown");

    // The structured log format used by the tracing macros.
    tracing::info_span!(
        "http_request",
        method = ?request.method(),
        uri = ?request.uri(),
        req_id = %request_id, 
    )
}
