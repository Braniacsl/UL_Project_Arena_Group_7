use std::env;

/// AppConfig
///
/// Holds the application's entire configuration state. This struct is designed to be
/// immutable once loaded, ensuring consistency across all threads and services (e.g., Repository, Storage).
/// It is pulled into the application state via FromRef, embodying the "immutable AppConfig"
/// part of the Unified State Pattern.
#[derive(Clone)]
pub struct AppConfig {
    // Database connection string (Postgres).
    pub db_url: String,
    // S3-compatible storage endpoint URL (MinIO in local, Supabase in prod).
    pub s3_endpoint: String,
    // S3 region (often a stub for local/Supabase).
    pub s3_region: String,
    // Access Key ID for S3-compatible storage.
    pub s3_key: String,
    // Secret Access Key for S3-compatible storage.
    pub s3_secret: String,
    // The bucket name used for all media uploads (videos, reports, images).
    pub s3_bucket: String,
    // Runtime environment marker. Controls feature activation (e.g., Dev Bypass).
    pub env: Env,
    // Secret key used to decode and validate incoming JWTs (Supabase-managed).
    pub jwt_secret: String,
}

/// Env
///
/// Defines the runtime context, used to switch between development utilities (MinIO, Bypass)
/// and secure, production-grade infrastructure (Supabase, Hardened Auth).
#[derive(Clone, PartialEq, Debug)]
pub enum Env {
    Local,
    Production,
}

impl Default for AppConfig {
    /// default
    ///
    /// Provides a safe, non-panicking AppConfig instance primarily used for test setup.
    /// This allows us to instantiate the configuration without needing to set environment
    /// variables for lightweight unit or integration testing state scaffolding.
    fn default() -> Self {
        // Provide safe, non-panicking dummy values for test state setup
        Self {
            db_url: "postgres://test_user:test_pass@localhost:5432/test_db".to_string(),
            // Default MinIO credentials for local/testing convenience.
            s3_endpoint: "http://localhost:9000".to_string(),
            s3_region: "us-east-1".to_string(),
            s3_key: "admin".to_string(),
            s3_secret: "password".to_string(),
            s3_bucket: "fyp-test".to_string(),
            env: Env::Local,
            jwt_secret: "super-secure-test-secret-value-local".to_string(),
        }
    }
}

impl AppConfig {
    /// load
    ///
    /// The canonical function for initializing the application configuration at startup.
    /// It reads all parameters from environment variables and implements the **fail-fast** principle.
    ///
    /// # Panics
    /// Panics if a critical environment variable required for the current runtime environment
    /// (especially Production) is not found. This prevents the application from starting
    /// with an incomplete or insecure configuration.
    pub fn load() -> Self {
        let env_str = env::var("APP_ENV").unwrap_or_else(|_| "local".to_string());
        let env = match env_str.as_str() {
            "production" => Env::Production,
            _ => Env::Local,
        };

        // JWT Secret Resolution
        // The production secret is mandatory and must be explicitly set.
        let jwt_secret = match env {
            Env::Production => env::var("SUPABASE_JWT_SECRET")
                .expect("FATAL: SUPABASE_JWT_SECRET must be set in production."),
            // In local, we provide a fallback, though the developer should ideally use the actual secret.
            _ => env::var("SUPABASE_JWT_SECRET")
                .unwrap_or_else(|_| "super-secure-test-secret-value-local".to_string()),
        };

        match env {
            Env::Local => Self {
                env: Env::Local,
                // DATABASE_URL must still be set, even in local environments (for MinIO/Docker DB).
                db_url: env::var("DATABASE_URL").expect("FATAL: DATABASE_URL required in local"),
                // Local storage (MinIO) uses hardcoded or known default credentials.
                s3_endpoint: "http://localhost:9000".to_string(),
                s3_region: "us-east-1".to_string(),
                s3_key: "admin".to_string(),
                s3_secret: "password".to_string(),
                s3_bucket: "fyp-uploads".to_string(),
                jwt_secret,
            },
            Env::Production => {
                // Production environment demands explicit setting of all infrastructure secrets.
                let project_url =
                    env::var("SUPABASE_URL").expect("FATAL: SUPABASE_URL required in prod");
                // Construct the S3 endpoint specifically for Supabase's Storage API gateway.
                let s3_endpoint = format!("{}/storage/v1/s3", project_url);

                Self {
                    env: Env::Production,
                    db_url: env::var("DATABASE_URL").expect("FATAL: DATABASE_URL required in prod"),
                    s3_endpoint,
                    // The region is often a stub when proxying through Supabase.
                    s3_region: "stub".to_string(),
                    s3_key: env::var("S3_ACCESS_KEY")
                        .expect("FATAL: S3_ACCESS_KEY required in prod"),
                    s3_secret: env::var("S3_SECRET_KEY")
                        .expect("FATAL: S3_SECRET_KEY required in prod"),
                    s3_bucket: env::var("S3_BUCKET_NAME")
                        .unwrap_or_else(|_| "fyp-uploads".to_string()),
                    jwt_secret,
                }
            }
        }
    }
}

