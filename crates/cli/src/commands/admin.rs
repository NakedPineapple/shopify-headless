//! Admin user management commands.
//!
//! # Usage
//!
//! ```bash
//! # Create an invite for a new admin (recommended)
//! np-cli admin invite -e admin@example.com -n "Admin Name" -r super_admin
//!
//! # Create a new admin user directly (without passkey)
//! np-cli admin create -e admin@example.com -n "Admin Name" -r super_admin
//! ```
//!
//! # Environment Variables
//!
//! - `ADMIN_DATABASE_URL` - `PostgreSQL` connection string for admin database
//! - `ADMIN_BASE_URL` - Base URL for generating setup links

use naked_pineapple_core::AdminRole;
use sqlx::PgPool;
use thiserror::Error;

/// Errors that can occur during admin operations.
#[derive(Debug, Error)]
pub enum AdminError {
    /// Required environment variable is missing.
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(&'static str),

    /// Database connection error.
    #[error("Database connection error: {0}")]
    Database(#[from] sqlx::Error),

    /// Invalid role.
    #[error("Invalid role: {0}. Valid roles: super_admin, admin")]
    InvalidRole(String),

    /// Invalid email.
    #[error("Invalid email: {0}")]
    InvalidEmail(String),

    /// User already exists.
    #[error("Admin user already exists with email: {0}")]
    UserExists(String),

    /// Invite already exists.
    #[error("Invite already exists for email: {0}")]
    InviteExists(String),
}

/// Create a new admin user.
///
/// # Arguments
///
/// * `email` - Admin's email address
/// * `name` - Admin's display name
/// * `role` - Admin's role (`super_admin`, `admin`, or `viewer`)
///
/// # Returns
///
/// The ID of the created admin user.
pub async fn create_user(email: &str, name: &str, role: &str) -> Result<i32, AdminError> {
    dotenvy::dotenv().ok();

    // Parse and validate role
    let role: AdminRole = role
        .parse()
        .map_err(|_| AdminError::InvalidRole(role.to_owned()))?;

    // Basic email validation
    if !email.contains('@') || !email.contains('.') {
        return Err(AdminError::InvalidEmail(email.to_owned()));
    }

    let database_url = std::env::var("ADMIN_DATABASE_URL")
        .map_err(|_| AdminError::MissingEnvVar("ADMIN_DATABASE_URL"))?;

    tracing::info!("Connecting to admin database...");
    let pool = PgPool::connect(&database_url).await?;

    tracing::info!("Creating admin user: {} ({})", email, role);

    // Check if user already exists
    let existing =
        sqlx::query_scalar!(r#"SELECT id FROM admin.admin_user WHERE email = $1"#, email)
            .fetch_optional(&pool)
            .await?;

    if existing.is_some() {
        return Err(AdminError::UserExists(email.to_owned()));
    }

    // Create the user
    let user_id = sqlx::query_scalar!(
        r#"
        INSERT INTO admin.admin_user (email, name, role)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
        email,
        name,
        role as AdminRole
    )
    .fetch_one(&pool)
    .await?;

    tracing::info!(
        "Admin user created successfully! ID: {}, Email: {}, Role: {}",
        user_id,
        email,
        role
    );
    tracing::warn!(
        "Note: User has no passkey. Use 'admin invite' instead to allow users to set up their own passkey."
    );

    Ok(user_id)
}

/// Create an invite for a new admin user.
///
/// # Arguments
///
/// * `email` - Email address to invite
/// * `name` - Admin's display name
/// * `role` - Admin's role (`super_admin` or `admin`)
/// * `expires_in_days` - Days until the invite expires
///
/// # Returns
///
/// The ID of the created invite.
pub async fn create_invite(
    email: &str,
    name: &str,
    role: &str,
    expires_in_days: i32,
) -> Result<i32, AdminError> {
    dotenvy::dotenv().ok();

    // Parse and validate role
    let role: AdminRole = role
        .parse()
        .map_err(|_| AdminError::InvalidRole(role.to_owned()))?;

    // Basic email validation
    if !email.contains('@') || !email.contains('.') {
        return Err(AdminError::InvalidEmail(email.to_owned()));
    }

    let database_url = std::env::var("ADMIN_DATABASE_URL")
        .map_err(|_| AdminError::MissingEnvVar("ADMIN_DATABASE_URL"))?;

    let base_url = std::env::var("ADMIN_BASE_URL").unwrap_or_else(|_| {
        tracing::warn!("ADMIN_BASE_URL not set, using default");
        "http://localhost:3001".to_owned()
    });

    tracing::info!("Connecting to admin database...");
    let pool = PgPool::connect(&database_url).await?;

    tracing::info!("Creating invite for: {} ({})", email, role);

    // Check if user already exists
    let existing =
        sqlx::query_scalar!(r#"SELECT id FROM admin.admin_user WHERE email = $1"#, email)
            .fetch_optional(&pool)
            .await?;

    if existing.is_some() {
        return Err(AdminError::UserExists(email.to_owned()));
    }

    // Check if invite already exists
    let existing_invite = sqlx::query_scalar!(
        r#"SELECT id FROM admin.admin_invite WHERE email = $1 AND used_at IS NULL"#,
        email
    )
    .fetch_optional(&pool)
    .await?;

    if existing_invite.is_some() {
        return Err(AdminError::InviteExists(email.to_owned()));
    }

    // Create the invite
    let invite_id = sqlx::query_scalar!(
        r#"
        INSERT INTO admin.admin_invite (email, name, role, expires_at)
        VALUES ($1, $2, $3, NOW() + make_interval(days => $4))
        RETURNING id
        "#,
        email,
        name,
        role as AdminRole,
        expires_in_days
    )
    .fetch_one(&pool)
    .await?;

    let setup_url = format!("{}/auth/setup", base_url.trim_end_matches('/'));

    tracing::info!("Invite created successfully!");
    tracing::info!("  Email: {}", email);
    tracing::info!("  Name: {}", name);
    tracing::info!("  Role: {}", role);
    tracing::info!("  Expires in: {} days", expires_in_days);
    tracing::info!("");
    tracing::info!("Share this setup link with the user:");
    tracing::info!("  {}", setup_url);
    tracing::info!("");
    tracing::info!(
        "They will need to enter their email ({}) to verify and create their passkey.",
        email
    );

    Ok(invite_id)
}
