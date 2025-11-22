use fyp_portal::storage::{MockStorageService, S3StorageClient, StorageService};
use uuid::Uuid;

#[cfg(test)]
mod mock_tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_success() {
        let mock = MockStorageService::new();
        let filename = "test.mp4";
        let result = mock.get_presigned_upload_url(filename, "video/mp4").await;
        assert!(result.is_ok());

        let url = result.unwrap();

        assert!(url.contains("signature=fake"));
        // Assertion changed: Check if the key is part of the returned URL
        assert!(url.contains(filename));
    }

    #[tokio::test]
    async fn test_mock_failure() {
        let mock = MockStorageService::new_failing();
        let result = mock.get_presigned_upload_url("test.mp4", "video/mp4").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_sanitization() {
        let mock = MockStorageService::new();
        let result = mock
            .get_presigned_upload_url("../../etc/passwd", "text/plain")
            .await;
        assert!(result.is_ok());

        let url = result.unwrap();

        // Assuming the sanitized key is embedded in the URL, this check confirms the sanitization.
        assert!(!url.contains(".."));
    }
}

#[cfg(test)]
mod s3_tests {
    use super::*;

    #[tokio::test]
    async fn test_s3_client_creation() {
        let _client = S3StorageClient::new(
            "http://localhost:9000",
            "testkey",
            "secret_key",
            "testsecret",
            "testbucket",
        );
        // Just testing that construction doesn't panic
    }

    #[tokio::test]
    async fn test_s3_presigned_url_format() {
        let client = S3StorageClient::new(
            "http://localhost:9000",
            "testkey",
            "secret_key",
            "testsecret",
            "testbucket",
        )
        .await;

        let key = format!("test-upload/report-{}.pdf", Uuid::new_v4());
        let result = client
            .get_presigned_upload_url(&key, "application/pdf")
            .await;

        // We expect this to succeed and return a URL
        assert!(result.is_ok());

        // FIX: The function only returns a single String (the URL).
        let url = result.unwrap();

        assert!(url.contains("localhost:9000"));
        // Assertion changed: Check if the key is part of the returned URL
        assert!(url.contains(&key));
    }
}
