use fyp_portal::{
    AppState,
    config::{AppConfig, Env},
    create_router,
    repository::{PostgresRepository, RepositoryState},
    storage::{S3StorageClient, StorageState},
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// main
///
/// The asynchronous entry point for the application, responsible for initializing
/// all core components: Configuration, Logging, Database, Storage, and the HTTP Server.
#[tokio::main]
async fn main() {
    // 1. Configuration & Environment Loading (Fail-Fast)
    // Loads .env file settings before configuration can be read.
    dotenv::dotenv().ok();
    // AppConfig::load() implements the fail-fast principle for missing Production secrets.
    let config = AppConfig::load();

    // 2. Logging Filter Setup
    // Sets the default log level. It prioritizes the RUST_LOG environment variable,
    // falling back to sensible defaults for local development.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "fyp_portal=debug,tower_http=info,axum=trace".into());

    // 3. Initialize Logging based on Environment (Production Observability)
    // The structured logging format is dynamically selected based on the APP_ENV.
    match config.env {
        Env::Local => {
            // LOCAL: Pretty print output for human readability during local debugging.
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
        Env::Production => {
            // PROD: JSON format output for ingestion by centralized log aggregators
            // (e.g., Datadog, AWS CloudWatch). This is essential for monitoring.
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
    }

    tracing::info!("Application starting in {:?} mode", config.env);

    // 4. Database Initialization (Postgres)
    // Creates a connection pool to the Postgres instance defined in the configuration.
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.db_url)
        .await
        .expect("FATAL: Failed to connect to Postgres. Check DATABASE_URL.");

    // Instantiate the Repository, wrapping it in an Arc for thread-safe sharing.
    let repo = Arc::new(PostgresRepository::new(pool)) as RepositoryState;

    // 5. Storage Initialization (S3/MinIO)
    // Instantiates the S3-compatible client using credentials resolved by AppConfig.
    let s3_client = S3StorageClient::new(
        &config.s3_endpoint,
        &config.s3_region,
        &config.s3_key,
        &config.s3_secret,
        &config.s3_bucket,
    )
    .await;

    // LOCAL-ONLY: Ensure the MinIO bucket is created if running locally.
    // This is a development convenience for the Dockerized setup.
    if config.env == Env::Local {
        use fyp_portal::storage::StorageService;
        s3_client.ensure_bucket_exists().await;
    }

    // Instantiate the Storage State, ready to be shared.
    let storage = Arc::new(s3_client) as StorageState;

    // 6. Unified State Assembly
    // Bundles all initialized dependencies into the shared AppState.
    let app_state = AppState {
        repo,
        storage,
        config,
    };

    // 7. Router and Server Startup
    let app = create_router(app_state);

    // Binds the TCP listener and initiates the HTTP server.
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("HTTP server bound successfully.");
    tracing::info!("Listening on 0.0.0.0:3000");
    tracing::info!("API Documentation (Swagger UI) available at: http://localhost:3000/swagger-ui");

    // The long-running Axum server process.
    axum::serve(listener, app).await.unwrap();
}

