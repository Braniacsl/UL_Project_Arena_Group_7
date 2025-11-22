/// Router Module Index
///
/// Organizes the application's routing logic into security-segregated modules,
/// enforcing a Defense-in-Depth strategy. This structure ensures that
/// access control is applied explicitly at the module level (via Axum layers),
/// preventing accidental exposure of protected endpoints.
///
/// The three modules map directly to the defined access roles.

/// Routes accessible to all users (anonymous, read-only).
/// Handlers must enforce visibility checks (`is_public=true`) at the Repository level.
pub mod public;

/// Routes protected by the `AuthUser` extractor middleware.
/// Requires a validated user session.
pub mod authenticated;

/// Routes restricted exclusively to users with the 'admin' role.
/// Implements mandatory authorization checks.
pub mod admin;

