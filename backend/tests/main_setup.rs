use fyp_portal::{AppConfig, config::Env};
use serial_test::serial;
use std::{env, panic};

// --- Setup/Teardown Utilities ---

/// Utility to run a test function and restore environment variables afterward
fn run_with_env<T, R>(test: T, cleanup_vars: Vec<&'static str>) -> R
where
    T: FnOnce() -> R + panic::UnwindSafe,
{
    // Save current environment variables
    let originals: Vec<(String, Option<String>)> = cleanup_vars
        .iter()
        .map(|&var| (var.to_string(), env::var(var).ok()))
        .collect();

    // Run the test
    let result = panic::catch_unwind(test);

    // Restore original environment variables
    for (key, original_value) in originals.into_iter().rev() {
        unsafe {
            if let Some(val) = original_value {
                env::set_var(&key, val);
            } else {
                env::remove_var(&key);
            }
        }
    }

    // Re-panic if the test failed
    match result {
        Ok(value) => value,
        Err(e) => panic::resume_unwind(e),
    }
}

// --- Tests ---

#[test]
#[serial]
fn test_app_config_production_fail_fast() {
    // We expect this to panic because we don't set the S3 secrets
    let result = panic::catch_unwind(|| {
        unsafe {
            env::set_var("APP_ENV", "production");
            env::set_var("DATABASE_URL", "postgres://user:pass@host/db");
            env::set_var("SUPABASE_URL", "http://fake-url.com");
        }
        // S3_ACCESS_KEY, S3_SECRET_KEY, and SUPABASE_JWT_SECRET are missing
        AppConfig::load()
    });

    // Cleanup
    let cleanup_vars = vec![
        "APP_ENV",
        "DATABASE_URL",
        "SUPABASE_URL",
        "S3_ACCESS_KEY",
        "S3_SECRET_KEY",
        "SUPABASE_JWT_SECRET",
    ];

    unsafe {
        for var in cleanup_vars {
            env::remove_var(var);
        }
    }

    // Assert that the config loading failed (panicked)
    assert!(
        result.is_err(),
        "Production config loading should panic on missing secrets"
    );
}

#[test]
#[serial]
fn test_app_config_local_env_defaults() {
    // Local mode should not panic, and should use hardcoded defaults
    let config = run_with_env(
        || {
            unsafe {
                env::set_var("APP_ENV", "local");
                env::set_var("DATABASE_URL", "postgres://user:pass@host/db");
                // Clear other variables to test fallbacks
                env::remove_var("SUPABASE_JWT_SECRET");
            }
            AppConfig::load()
        },
        vec!["APP_ENV", "DATABASE_URL", "SUPABASE_JWT_SECRET"],
    );

    assert_eq!(config.env, Env::Local);
    // Check hardcoded MinIO default
    assert_eq!(config.s3_endpoint, "http://localhost:9000");
    // Check local JWT secret fallback
    assert_eq!(config.jwt_secret, "super-secure-test-secret-value-local");
}

