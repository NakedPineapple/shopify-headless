//! Domain classifier using Claude Haiku.
//!
//! The classifier takes a user query and returns 1-3 relevant domains.
//! This is the first stage of tool selection, designed to be fast and cheap.

use std::fmt::Write;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use super::{DOMAIN_DESCRIPTIONS, DOMAINS, ToolSelectionError};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const HAIKU_MODEL: &str = "claude-3-5-haiku-latest";
const MAX_TOKENS: u32 = 100;

/// Domain classifier using Claude Haiku for fast categorization.
#[derive(Clone)]
pub struct DomainClassifier {
    client: reqwest::Client,
}

impl DomainClassifier {
    /// Create a new domain classifier.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key
    ///
    /// # Panics
    ///
    /// Panics if the API key contains invalid header characters.
    #[must_use]
    pub fn new(api_key: &secrecy::SecretString) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(api_key.expose_secret()).expect("Invalid API key for header"),
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build HTTP client");

        Self { client }
    }

    /// Classify a user query into 1-3 relevant domains.
    ///
    /// # Arguments
    ///
    /// * `query` - The user's query text
    ///
    /// # Returns
    ///
    /// A vector of 1-3 domain names.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns an invalid response.
    #[instrument(skip(self, query), fields(query_len = query.len()))]
    pub async fn classify(&self, query: &str) -> Result<Vec<String>, ToolSelectionError> {
        let system_prompt = build_system_prompt();
        let user_message = format!(
            "Classify this query into 1-3 relevant domains. Return ONLY the domain names, comma-separated.\n\nQuery: {query}"
        );

        let request = ClassifyRequest {
            model: HAIKU_MODEL.to_string(),
            max_tokens: MAX_TOKENS,
            system: Some(system_prompt),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_message,
            }],
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ToolSelectionError::Classification(format!(
                "Haiku API error ({status}): {body}"
            )));
        }

        let response: ClassifyResponse = response.json().await?;

        let content = response
            .content
            .into_iter()
            .map(|block| match block {
                ContentBlock::Text { text } => text,
            })
            .next()
            .ok_or_else(|| {
                ToolSelectionError::InvalidResponse("No text content in response".to_string())
            })?;

        // Parse the comma-separated domains
        let domains = parse_domains(&content);

        if domains.is_empty() {
            debug!(
                response = %content,
                "No valid domains parsed, falling back to general domains"
            );
            // Fall back to most common domains
            return Ok(vec!["orders".to_string(), "customers".to_string()]);
        }

        debug!(domains = ?domains, "Classified query into domains");
        Ok(domains)
    }
}

/// Build the system prompt for domain classification.
fn build_system_prompt() -> String {
    let mut prompt = String::from(
        "You are a classifier that categorizes e-commerce admin queries into domains.\n\n\
         Available domains:\n",
    );

    for (domain, description) in DOMAIN_DESCRIPTIONS {
        let _ = writeln!(prompt, "- {domain}: {description}");
    }

    prompt.push_str(
        "\nRules:\n\
         1. Return 1-3 most relevant domains\n\
         2. Return ONLY domain names, comma-separated (e.g., \"orders, customers\")\n\
         3. Most queries need only 1-2 domains\n\
         4. Choose based on what data/actions the query requires",
    );

    prompt
}

/// Parse comma-separated domain names from classifier response.
fn parse_domains(response: &str) -> Vec<String> {
    response
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| DOMAINS.contains(&s.as_str()))
        .take(3) // Limit to 3 domains
        .collect()
}

/// Request body for Claude classification.
#[derive(Debug, Serialize)]
struct ClassifyRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<Message>,
}

/// A message in the request.
#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

/// Response from Claude API.
#[derive(Debug, Deserialize)]
struct ClassifyResponse {
    content: Vec<ContentBlock>,
}

/// Content block in response.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_domains_single() {
        let result = parse_domains("orders");
        assert_eq!(result, vec!["orders"]);
    }

    #[test]
    fn test_parse_domains_multiple() {
        let result = parse_domains("orders, customers, products");
        assert_eq!(result, vec!["orders", "customers", "products"]);
    }

    #[test]
    fn test_parse_domains_with_whitespace() {
        let result = parse_domains("  orders  ,  customers  ");
        assert_eq!(result, vec!["orders", "customers"]);
    }

    #[test]
    fn test_parse_domains_filters_invalid() {
        let result = parse_domains("orders, invalid_domain, customers");
        assert_eq!(result, vec!["orders", "customers"]);
    }

    #[test]
    fn test_parse_domains_max_three() {
        let result = parse_domains("orders, customers, products, inventory, discounts");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_parse_domains_case_insensitive() {
        let result = parse_domains("ORDERS, Customers");
        assert_eq!(result, vec!["orders", "customers"]);
    }

    #[test]
    fn test_build_system_prompt_contains_domains() {
        let prompt = build_system_prompt();
        for domain in DOMAINS {
            assert!(prompt.contains(domain));
        }
    }
}
