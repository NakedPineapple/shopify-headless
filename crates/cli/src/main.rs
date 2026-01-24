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
//! ```
//!
//! # Future Commands
//!
//! - `migrate` - Run database migrations
//! - `seed` - Seed database with test data
//! - `user create` - Create admin users
//! - `cache clear` - Clear API caches

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
    // TODO: Add more commands
    // /// Seed database with test data
    // Seed {
    //     #[command(subcommand)]
    //     target: SeedTarget,
    // },
    // /// Manage admin users
    // User {
    //     #[command(subcommand)]
    //     action: UserAction,
    // },
}

#[derive(Subcommand)]
enum MigrateTarget {
    /// Run storefront database migrations
    Storefront,
    /// Run admin database migrations
    Admin,
    /// Run all database migrations
    All,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Migrate { target } => match target {
            MigrateTarget::Storefront => commands::migrate::storefront().await,
            MigrateTarget::Admin => commands::migrate::admin().await,
            MigrateTarget::All => {
                if let Err(e) = commands::migrate::storefront().await {
                    Err(e)
                } else {
                    commands::migrate::admin().await
                }
            }
        },
    };

    if let Err(e) = result {
        tracing::error!("Migration failed: {e}");
        std::process::exit(1);
    }
}
