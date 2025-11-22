use async_trait::async_trait;
use aws_sdk_s3 as s3;
use s3::presigning::PresigningConfig;
use std::sync::Arc;
use std::time::Duration;

// 1. StorageService Contract
/// StorageService
///
/// Defines the abstract contract for all interactions with the object storage layer.
/// This trait allows us to swap the concrete implementation—from the real S3 client
/// (S3StorageClient) in production to the in-memory Mock (MockStorageService) during
/// testing—without affecting the calling handlers.
#[async_trait]
pub trait StorageService: Send + Sync {
    /// Ensures the configured bucket exists. Used primarily in the `Env::Local` setup
    /// to automatically provision the required bucket in MinIO. No-op in production.
    async fn ensure_bucket_exists(&self);

    /// Generates a temporary, cryptographically signed URL allowing a client to upload
    /// a file directly to the S3 bucket.
    ///
    /// The URL generated includes constraints on expiration time and content type.
    ///
    /// # Arguments
    /// * `key`: The final object key (path + filename) in the S3 bucket.
    /// * `content_type`: The expected MIME type (e.g., "video/mp4").
    async fn get_presigned_upload_url(
        &self,
        key: &str,
        content_type: &str,
    ) -> Result<String, String>;
}

// 2. The Real Implementation (S3/MinIO/Supabase)
/// S3StorageClient
///
/// The concrete implementation using the AWS SDK for S3. Due to S3 compatibility,
/// this client transparently handles connections to:
/// - **Local:** Dockerized MinIO instance.
/// - **Production:** Supabase Storage endpoint.
///
/// The `force_path_style(true)` is critical for MinIO and Supabase compatibility.
#[derive(Clone)]
pub struct S3StorageClient {
    client: s3::Client,
    bucket_name: String,
}

impl S3StorageClient {
    /// new
    ///
    /// Constructs the S3 client using credentials and configuration from AppConfig.
    pub async fn new(
        endpoint: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
        bucket: &str,
    ) -> Self {
        let credentials =
            s3::config::Credentials::new(access_key, secret_key, None, None, "static");

        let config = s3::Config::builder()
            .credentials_provider(credentials)
            .endpoint_url(endpoint)
            .region(s3::config::Region::new(region.to_string()))
            .behavior_version_latest()
            // CRITICAL: Forces the client to use path-style addressing (e.g., http://endpoint/bucket/key)
            // which is required for MinIO and Supabase Storage API gateways.
            .force_path_style(true)
            .build();

        let client = s3::Client::from_conf(config);

        Self {
            client,
            bucket_name: bucket.to_string(),
        }
    }
}

#[async_trait]
impl StorageService for S3StorageClient {
    /// ensure_bucket_exists
    ///
    /// Calls the S3 CreateBucket API. Since S3 APIs are idempotent, this only creates
    /// the bucket if it does not already exist. It's safe to call at startup.
    async fn ensure_bucket_exists(&self) {
        let _ = self
            .client
            .create_bucket()
            .bucket(&self.bucket_name)
            .send()
            .await;
    }

    /// get_presigned_upload_url
    ///
    /// Implements the secure part of the Media Pipeline.
    async fn get_presigned_upload_url(
        &self,
        key: &str,
        content_type: &str,
    ) -> Result<String, String> {
        // Expiration constrained to 10 minutes (600 seconds) as per security review.
        let expires_in = Duration::from_secs(600);

        let presigned_req = self
            .client
            .put_object()
            .bucket(&self.bucket_name)
            .key(key)
            // CRITICAL SECURITY: Forces the client request to include this Content-Type header.
            .content_type(content_type)
            .presigned(PresigningConfig::expires_in(expires_in).unwrap())
            .await
            .map_err(|e| e.to_string())?;

        Ok(presigned_req.uri().to_string())
    }
}

/// sanitize_key
///
/// Utility function to prevent path traversal attacks by removing directory
/// navigation components (e.g., `..`, `.`) from a user-provided key segment.
fn sanitize_key(key: &str) -> String {
    key.split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".." && *segment != ".")
        .collect::<Vec<_>>()
        .join("/")
}

// 3. The Mock Implementation (For Unit Tests)
/// MockStorageService
///
/// A mock implementation of `StorageService` used exclusively for unit and integration testing.
/// This allows us to test the `get_presigned_url` handler logic without requiring a network
/// connection to S3, isolating the test boundary.
#[derive(Clone)]
pub struct MockStorageService {
    /// When true, all operations return a simulated failure.
    pub should_fail: bool,
}

impl MockStorageService {
    pub fn new() -> Self {
        Self { should_fail: false }
    }

    pub fn new_failing() -> Self {
        Self { should_fail: true }
    }
}

#[async_trait]
impl StorageService for MockStorageService {
    async fn ensure_bucket_exists(&self) {
        // No-op in mock environment.
    }

    async fn get_presigned_upload_url(
        &self,
        key: &str,
        _content_type: &str,
    ) -> Result<String, String> {
        if self.should_fail {
            return Err("Mock Storage Error: Simulation requested".to_string());
        }

        let sanitized_key = sanitize_key(key);

        // Returns a deterministic, local-style URL for mock assertions.
        Ok(format!(
            "http://localhost:9000/mock-bucket/{}?signature=fake",
            sanitized_key
        ))
    }
}

/// StorageState
///
/// The concrete type used to share the storage service access across the application state.
pub type StorageState = Arc<dyn StorageService>;

