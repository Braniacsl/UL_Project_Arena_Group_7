use axum::{
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, header, request::Parts},
};
use jsonwebtoken::{DecodingKey, Validation, decode, errors::ErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    config::{AppConfig, Env},
    repository::RepositoryState,
};

/// Claims
///
/// Represents the standard payload structure expected inside a JSON Web Token (JWT).
/// These claims are signed by the server's secret and validated upon every authenticated request.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (sub): The UUID of the user. This is the primary key used to fetch
    /// the user's details and role from the public.profiles table.
    pub sub: Uuid,
    /// Expiration Time (exp): Timestamp after which the JWT must not be accepted.
    /// This is crucial for preventing replay attacks and maintaining session freshness.
    pub exp: usize,
    /// Issued At (iat): Timestamp when the JWT was issued.
    pub iat: usize,
}

/// AuthUser Extractor Result
///
/// This struct represents the resolved identity of an authenticated request.
/// It is the core output of the AuthUser extractor implementation.
/// Handlers will use this struct to retrieve the user's ID and verify permissions.
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// The unique identifier of the user, mapped to auth.users.id and public.profiles.id.
    pub id: Uuid,
    /// The user's role, primarily 'student' or 'admin'. Used for Role-Based Access Control (RBAC).
    pub role: String,
}

/// AuthUser Extractor Implementation
///
/// Implements Axum's FromRequestParts trait, making AuthUser usable as a function argument
/// in any authenticated handler. This is a crucial piece of our Clean Architecture
/// strategy, as it cleanly separates authentication (middleware/extractor) from
/// business logic (the handler).
///
/// The entire process involves:
/// 1. Dependency Resolution: Accessing Repository and AppConfig from the application state.
/// 2. Local Bypass: Allowing development-time access using the 'x-user-id' header.
/// 3. Token Validation: Standard Bearer token extraction and JWT decoding.
/// 4. DB Lookup: Fetching the user's current role and existence from PostgreSQL.
///
/// Rejection: Returns StatusCode::UNAUTHORIZED (401) on any failure.
impl<S> FromRequestParts<S> for AuthUser
where
    // S must allow sending across threads and sharing.
    S: Send + Sync,
    // Allows the extractor to pull the Repository State from the app state.
    RepositoryState: FromRef<S>,
    // Allows the extractor to pull the AppConfig (for JWT secret and Env check).
    AppConfig: FromRef<S>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // 1. Dependency Resolution
        let repo = RepositoryState::from_ref(state);
        let config = AppConfig::from_ref(state);

        // 2. Local Development Bypass Check
        // If the application is running in Env::Local, we allow authentication by
        // providing a known, valid UUID in the 'x-user-id' header.
        // This accelerates development but is guarded by the Env check.
        if config.env == Env::Local {
            if let Some(user_id_header) = parts.headers.get("x-user-id") {
                if let Ok(id_str) = user_id_header.to_str() {
                    // Attempt to parse the header value as a UUID.
                    if let Ok(user_id) = Uuid::parse_str(id_str) {
                        // Crucially, we verify that this UUID maps to an actual user/profile
                        // in the local development database to ensure roles are correctly loaded.
                        if let Some(user) = repo.get_user(user_id).await {
                            return Ok(AuthUser {
                                id: user.id,
                                role: user.role,
                            });
                        }
                    }
                }
            }
        }
        // If Env is Production, or if the bypass failed (e.g., header was bad or user not found),
        // execution falls through to the standard JWT validation flow.

        // 3. Token Extraction
        // Attempt to retrieve the Authorization header and ensure it is prefixed with "Bearer ".
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // 4. JWT Decoding Setup
        let secret = &config.jwt_secret;
        let decoding_key = DecodingKey::from_secret(secret.as_bytes());

        let mut validation = Validation::default();

        // Ensure expiration time validation is always active.
        validation.validate_exp = true;

        // 5. Decode and Validate the Token
        let token_data = match decode::<Claims>(token, &decoding_key, &validation) {
            Ok(data) => data,
            Err(e) => {
                // Detailed error inspection: Crucial for security and logging.
                match e.kind() {
                    // Token expired: This is the most common failure for a valid-but-old token.
                    ErrorKind::ExpiredSignature => return Err(StatusCode::UNAUTHORIZED),
                    // Catch all other failure types (bad signature, malformed token, etc.).
                    _ => return Err(StatusCode::UNAUTHORIZED),
                }
            }
        };

        let user_id = token_data.claims.sub;

        // 6. Database Lookup (Final Verification)
        // Check the database for the user's existence and retrieve their current role.
        // This prevents access if the user was deleted after the token was issued.
        let user = repo
            .get_user(user_id)
            .await
            // If the user is not found, the token is technically valid but the user is not active.
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Success: Return the resolved identity.
        Ok(AuthUser {
            id: user.id,
            role: user.role,
        })
    }
}

