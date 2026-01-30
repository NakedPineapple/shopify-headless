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
//! # Create an invite for a new admin (recommended)
//! np-cli admin invite -e admin@example.com -n "Admin Name" -r super_admin
//!
//! # Create admin user directly (no passkey)
//! np-cli admin create -e admin@example.com -n "Admin Name" -r super_admin
//!
//! # Seed tool examples for AI chat
//! np-cli seed tool-examples --file crates/admin/data/tool_examples.yaml
//!
//! # Show tool examples statistics
//! np-cli seed tool-examples-stats
//! ```
//!
//! # Commands
//!
//! - `migrate` - Run database migrations
//! - `migrate rollback` - Rollback database migrations
//! - `admin invite` - Create invite for new admin (recommended)
//! - `admin create` - Create admin user directly (no passkey)
//! - `seed tool-examples` - Seed tool example queries for AI chat
//! - `seed tool-examples-stats` - Show tool examples statistics

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
    /// Seed database with data
    Seed {
        #[command(subcommand)]
        action: SeedAction,
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
    /// Create a new admin user (requires passkey registered separately)
    Create {
        /// Admin email address
        #[arg(short, long)]
        email: String,

        /// Admin display name
        #[arg(short, long)]
        name: String,

        /// Admin role (`super_admin`, `admin`)
        #[arg(short, long, default_value = "admin")]
        role: String,
    },
    /// Create an invite for a new admin (recommended)
    Invite {
        /// Email address to invite
        #[arg(short, long)]
        email: String,

        /// Admin display name
        #[arg(short, long)]
        name: String,

        /// Admin role (`super_admin`, `admin`)
        #[arg(short, long, default_value = "admin")]
        role: String,

        /// Days until invite expires
        #[arg(short = 'x', long, default_value = "7")]
        expires_in_days: i32,
    },
}

#[derive(Subcommand)]
enum SeedAction {
    /// Seed tool example queries for AI chat embedding-based selection
    ToolExamples {
        /// Path to YAML file containing tool examples
        #[arg(short, long)]
        file: String,

        /// Clear existing pre-seeded examples before inserting
        #[arg(short, long, default_value = "false")]
        clear: bool,
    },
    /// Show statistics about existing tool examples
    ToolExamplesStats,
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
            AdminAction::Invite {
                email,
                name,
                role,
                expires_in_days,
            } => {
                commands::admin::create_invite(&email, &name, &role, expires_in_days).await?;
            }
        },
        Commands::Seed { action } => match action {
            SeedAction::ToolExamples { file, clear } => {
                commands::seed::tool_examples(&file, clear).await?;
            }
            SeedAction::ToolExamplesStats => {
                commands::seed::tool_examples_stats().await?;
            }
        },
    }
    Ok(())
}
