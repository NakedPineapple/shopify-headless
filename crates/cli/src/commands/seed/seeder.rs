//! Pre-seed tool example queries from YAML configuration.
//!
//! This module reads example queries from a YAML file and inserts them into
//! the `tool_example_queries` table with their embeddings. This provides the
//! initial data for embedding-based tool selection.
//!
//! ## YAML Format
//!
//! ```yaml
//! get_orders:
//!   domain: orders
//!   examples:
//!     - "Show me recent orders"
//!     - "What orders came in today?"
//!
//! cancel_order:
//!   domain: orders
//!   examples:
//!     - "Cancel order #1001"
//!     - "I need to cancel this order"
//! ```

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};

use naked_pineapple_services::openai::EmbeddingClient;

use super::db::{self, CreateToolExample};
use super::error::SeedError;

/// Maximum batch size for embedding requests.
const EMBEDDING_BATCH_SIZE: usize = 50;

/// Available tool domains (mirrors admin's `DOMAINS` constant).
const DOMAINS: &[&str] = &[
    "analytics",
    "orders",
    "customers",
    "products",
    "inventory",
    "collections",
    "discounts",
    "gift_cards",
    "fulfillment",
    "finance",
    "order_editing",
];

/// Configuration for a single tool's examples.
#[derive(Debug, Deserialize)]
pub struct ToolExampleConfig {
    /// Domain this tool belongs to.
    pub domain: String,
    /// Example queries that should map to this tool.
    pub examples: Vec<String>,
}

/// Full configuration file structure.
pub type ToolExamplesConfig = HashMap<String, ToolExampleConfig>;

/// Result of seeding operation.
#[derive(Debug)]
pub struct SeedResult {
    /// Number of examples inserted.
    pub inserted: u64,
    /// Number of examples skipped (already exist).
    pub skipped: u64,
    /// Number of tools processed.
    pub tools_processed: usize,
    /// Errors encountered (`tool_name`, error message).
    pub errors: Vec<(String, String)>,
}

/// Seed tool examples from a YAML file.
///
/// Reads the configuration, generates embeddings via `OpenAI`, and inserts
/// into the database.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed, or if database
/// operations fail.
#[instrument(skip(pool, embeddings), fields(path = %path.as_ref().display()))]
pub async fn seed_from_file<P: AsRef<Path>>(
    pool: &PgPool,
    embeddings: &EmbeddingClient,
    path: P,
    clear_existing: bool,
) -> Result<SeedResult, SeedError> {
    let path = path.as_ref();

    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| SeedError::Io(format!("Failed to read {}: {e}", path.display())))?;

    let config: ToolExamplesConfig = serde_yaml_ng::from_str(&content)
        .map_err(|e| SeedError::Config(format!("Failed to parse YAML: {e}")))?;

    seed_from_config(pool, embeddings, config, clear_existing).await
}

/// Seed tool examples from a configuration struct.
///
/// # Errors
///
/// Returns an error if embedding generation or database operations fail.
#[instrument(skip(pool, embeddings, config), fields(tools = config.len()))]
async fn seed_from_config(
    pool: &PgPool,
    embeddings: &EmbeddingClient,
    config: ToolExamplesConfig,
    clear_existing: bool,
) -> Result<SeedResult, SeedError> {
    if clear_existing {
        let deleted = db::delete_preseeded(pool).await?;
        info!(deleted, "Cleared existing pre-seeded examples");
    }

    let mut result = SeedResult {
        inserted: 0,
        skipped: 0,
        tools_processed: 0,
        errors: Vec::new(),
    };

    let mut all_examples: Vec<(String, String, String)> = Vec::new();

    for (tool_name, tool_config) in &config {
        for example in &tool_config.examples {
            all_examples.push((
                tool_name.clone(),
                tool_config.domain.clone(),
                example.clone(),
            ));
        }
    }

    info!(
        total_examples = all_examples.len(),
        "Processing tool examples"
    );

    for chunk in all_examples.chunks(EMBEDDING_BATCH_SIZE) {
        if let Err(e) = process_batch(pool, embeddings, chunk, &mut result).await {
            warn!(error = %e, "Batch processing error");
        }
    }

    result.tools_processed = config.len();

    info!(
        inserted = result.inserted,
        skipped = result.skipped,
        tools = result.tools_processed,
        errors = result.errors.len(),
        "Seeding complete"
    );

    Ok(result)
}

/// Process a batch of examples.
async fn process_batch(
    pool: &PgPool,
    embeddings: &EmbeddingClient,
    batch: &[(String, String, String)],
    result: &mut SeedResult,
) -> Result<(), SeedError> {
    let mut to_embed: Vec<(usize, &str)> = Vec::new();

    for (i, (tool_name, _, query)) in batch.iter().enumerate() {
        let exists = db::example_exists(pool, tool_name, query).await?;
        if exists {
            result.skipped += 1;
            debug!(tool = %tool_name, query = %query, "Skipping existing example");
        } else {
            to_embed.push((i, query.as_str()));
        }
    }

    if to_embed.is_empty() {
        return Ok(());
    }

    let queries: Vec<&str> = to_embed.iter().map(|(_, q)| *q).collect();
    let batch_embeddings = embeddings.embed_batch(&queries).await?;

    for ((orig_idx, _), embedding) in to_embed.iter().zip(batch_embeddings) {
        let Some((tool_name, domain, query)) = batch.get(*orig_idx) else {
            continue;
        };

        let params = CreateToolExample {
            tool_name: tool_name.clone(),
            domain: domain.clone(),
            example_query: query.clone(),
            embedding,
            is_learned: false,
        };

        match db::insert_tool_example(pool, params).await {
            Ok(_) => {
                result.inserted += 1;
                debug!(tool = %tool_name, query = %query, "Inserted example");
            }
            Err(e) => {
                result.errors.push((tool_name.clone(), e.to_string()));
                warn!(tool = %tool_name, error = %e, "Failed to insert example");
            }
        }
    }

    Ok(())
}

/// Validate tool examples configuration.
///
/// Checks that domains are valid and examples are non-empty. Tool name
/// validation is deferred to runtime when the admin crate loads tools.
#[must_use]
pub fn validate_config(config: &ToolExamplesConfig) -> Vec<String> {
    let mut errors = Vec::new();

    for (tool_name, tool_config) in config {
        if !DOMAINS.contains(&tool_config.domain.as_str()) {
            errors.push(format!(
                "Invalid domain '{}' for tool '{tool_name}'",
                tool_config.domain
            ));
        }

        if tool_config.examples.is_empty() {
            errors.push(format!("No examples provided for tool: {tool_name}"));
        }

        for (i, example) in tool_config.examples.iter().enumerate() {
            if example.trim().is_empty() {
                errors.push(format!(
                    "Empty example string at index {i} for tool: {tool_name}"
                ));
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml_config() {
        let yaml = r#"
get_orders:
  domain: orders
  examples:
    - "Show me recent orders"
    - "What orders came in today?"

cancel_order:
  domain: orders
  examples:
    - "Cancel order #1001"
"#;

        let config: ToolExamplesConfig = serde_yaml_ng::from_str(yaml).expect("valid YAML");
        assert_eq!(config.len(), 2);
        let get_orders = config.get("get_orders").expect("get_orders exists");
        assert_eq!(get_orders.domain, "orders");
        assert_eq!(get_orders.examples.len(), 2);
        let cancel_order = config.get("cancel_order").expect("cancel_order exists");
        assert_eq!(cancel_order.examples.len(), 1);
    }

    #[test]
    fn test_validate_config_invalid_domain() {
        let mut config = ToolExamplesConfig::new();
        config.insert(
            "test_tool".to_string(),
            ToolExampleConfig {
                domain: "invalid_domain".to_string(),
                examples: vec!["test query".to_string()],
            },
        );
        let errors = validate_config(&config);
        assert!(!errors.is_empty());
        assert!(
            errors
                .first()
                .expect("should have error")
                .contains("Invalid domain")
        );
    }

    #[test]
    fn test_validate_config_empty_examples() {
        let mut config = ToolExamplesConfig::new();
        config.insert(
            "test_tool".to_string(),
            ToolExampleConfig {
                domain: "orders".to_string(),
                examples: vec![],
            },
        );
        let errors = validate_config(&config);
        assert!(!errors.is_empty());
        assert!(
            errors
                .first()
                .expect("should have error")
                .contains("No examples")
        );
    }

    #[test]
    fn test_validate_config_valid() {
        let mut config = ToolExamplesConfig::new();
        config.insert(
            "get_orders".to_string(),
            ToolExampleConfig {
                domain: "orders".to_string(),
                examples: vec!["Show me recent orders".to_string()],
            },
        );
        let errors = validate_config(&config);
        assert!(errors.is_empty());
    }
}
