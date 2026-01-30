//! Seed database with tool example queries for AI chat.
//!
//! This command reads example queries from a YAML file, generates embeddings
//! via `OpenAI`, and inserts them into the `tool_example_queries` table for
//! embedding-based tool selection.

use std::path::Path;

use secrecy::SecretString;
use tracing::{error, info};

use naked_pineapple_admin::db;
use naked_pineapple_admin::tool_selection::{
    EmbeddingClient, ToolExamplesConfig, seed_from_file, validate_config,
};

/// Seed tool examples from a YAML file.
///
/// # Arguments
///
/// * `file_path` - Path to the YAML configuration file
/// * `clear_existing` - If true, clear existing pre-seeded examples first
///
/// # Errors
///
/// Returns an error if environment variables are missing, file cannot be read,
/// or database operations fail.
pub async fn tool_examples(
    file_path: &str,
    clear_existing: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Get required environment variables
    let database_url = std::env::var("ADMIN_DATABASE_URL")
        .map(SecretString::from)
        .map_err(|_| "ADMIN_DATABASE_URL not set")?;

    let openai_api_key = std::env::var("OPENAI_API_KEY")
        .map(SecretString::from)
        .map_err(|_| "OPENAI_API_KEY not set")?;

    // Verify file exists
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}").into());
    }

    info!(path = %file_path, "Loading tool examples from file");

    // Read and validate YAML before connecting to database
    let content = tokio::fs::read_to_string(path).await?;
    let config: ToolExamplesConfig = serde_yaml::from_str(&content)?;

    info!(tools = config.len(), "Parsed configuration");

    // Validate configuration
    let errors = validate_config(&config);
    if !errors.is_empty() {
        error!("Configuration validation failed:");
        for err in &errors {
            error!("  - {err}");
        }
        return Err(format!("{} validation errors found", errors.len()).into());
    }

    info!("Configuration validated successfully");

    // Connect to database
    let pool = db::create_pool(&database_url).await?;
    info!("Connected to database");

    // Create embedding client
    let embeddings = EmbeddingClient::new(&openai_api_key);

    // Seed examples
    info!(clear_existing, "Starting seeding process");
    let result = seed_from_file(&pool, &embeddings, path, clear_existing).await?;

    // Print summary
    info!("Seeding complete!");
    info!("  Tools processed: {}", result.tools_processed);
    info!("  Examples inserted: {}", result.inserted);
    info!("  Examples skipped (already exist): {}", result.skipped);

    if !result.errors.is_empty() {
        error!("  Errors: {}", result.errors.len());
        for (tool, err) in &result.errors {
            error!("    - {tool}: {err}");
        }
    }

    Ok(())
}

/// Show statistics about existing tool examples.
///
/// # Errors
///
/// Returns an error if database connection fails.
pub async fn tool_examples_stats() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    let database_url = std::env::var("ADMIN_DATABASE_URL")
        .map(SecretString::from)
        .map_err(|_| "ADMIN_DATABASE_URL not set")?;

    let pool = db::create_pool(&database_url).await?;

    let total = naked_pineapple_admin::db::tool_examples::get_total_count(&pool).await?;
    let by_domain = naked_pineapple_admin::db::tool_examples::get_domain_counts(&pool).await?;

    info!("Tool Examples Statistics");
    info!("========================");
    info!("Total examples: {total}");
    info!("By domain:");

    for domain_count in by_domain {
        info!("  {}: {}", domain_count.domain, domain_count.count);
    }

    Ok(())
}
