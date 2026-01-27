//! Naked Pineapple CLI - Database migrations and management tools.
//!
//! # Usage
//!
//! ```bash
//! # Run storefront database migrations
//! np-cli migrate storefront
//!
//! # Run admin database migrations
//! np-cli migrate admin
//!
//! # Run all database migrations
//! np-cli migrate all
//!
//! # Rollback storefront migrations (1 by default)
//! np-cli migrate rollback storefront
//!
//! # Rollback multiple migrations
//! np-cli migrate rollback storefront --count 3
//!
//! # Create admin user
//! np-cli admin create -e admin@example.com -n "Admin Name" -r super_admin
//! ```
//!
//! # Commands
//!
//! - `migrate` - Run database migrations
//! - `migrate rollback` - Rollback database migrations
//! - `admin create` - Create admin users
//! - `seed` - Seed database with test data (TODO)

#![cfg_attr(not(test), forbid(unsafe_code))]

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(name = "np-cli")]
#[command(author, version, about = "Naked Pineapple CLI tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run database migrations
    Migrate {
        #[command(subcommand)]
        target: MigrateTarget,
    },
    /// Manage admin users
    Admin {
        #[command(subcommand)]
        action: AdminAction,
    },
}

#[derive(Subcommand)]
enum MigrateTarget {
    /// Run storefront database migrations
    Storefront,
    /// Run admin database migrations
    Admin,
    /// Run all database migrations
    All,
    /// Rollback migrations
    Rollback {
        #[command(subcommand)]
        target: RollbackTarget,
        /// Number of migrations to roll back
        #[arg(short, long, default_value = "1", global = true)]
        count: i64,
    },
}

#[derive(Subcommand)]
enum RollbackTarget {
    /// Rollback storefront database migrations
    Storefront,
    /// Rollback admin database migrations
    Admin,
}

#[derive(Subcommand)]
enum AdminAction {
    /// Create a new admin user
    Create {
        /// Admin email address
        #[arg(short, long)]
        email: String,

        /// Admin display name
        #[arg(short, long)]
        name: String,

        /// Admin role (`super_admin`, `admin`, `viewer`)
        #[arg(short, long, default_value = "admin")]
        role: String,
    },
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result: Result<(), Box<dyn std::error::Error>> = run(cli).await;

    if let Err(e) = result {
        tracing::error!("Command failed: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Migrate { target } => match target {
            MigrateTarget::Storefront => commands::migrate::storefront().await?,
            MigrateTarget::Admin => commands::migrate::admin().await?,
            MigrateTarget::All => {
                commands::migrate::storefront().await?;
                commands::migrate::admin().await?;
            }
            MigrateTarget::Rollback { target, count } => match target {
                RollbackTarget::Storefront => {
                    commands::migrate::rollback_storefront(count).await?;
                }
                RollbackTarget::Admin => {
                    commands::migrate::rollback_admin(count).await?;
                }
            },
        },
        Commands::Admin { action } => match action {
            AdminAction::Create { email, name, role } => {
                commands::admin::create_user(&email, &name, &role).await?;
            }
        },
    }
    Ok(())
}
