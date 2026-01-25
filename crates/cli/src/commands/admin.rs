//! Admin user management commands.
//!
//! # Usage
//!
//! ```bash
//! # Create a new admin user
//! np-cli admin create -e admin@example.com -n "Admin Name" -r super_admin
//! ```
//!
//! # Environment Variables
//!
//! - `ADMIN_DATABASE_URL` - `PostgreSQL` connection string for admin database

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
    #[error("Invalid role: {0}. Valid roles: super_admin, admin, viewer")]
    InvalidRole(String),

    /// Invalid email.
    #[error("Invalid email: {0}")]
    InvalidEmail(String),

    /// User already exists.
    #[error("Admin user already exists with email: {0}")]
    UserExists(String),
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
    tracing::info!("The user can now log in at the admin panel and register their passkey.");

    Ok(user_id)
}
