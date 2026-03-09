//! Claude tool definitions and system prompts for the email triage pipeline.
//!
//! Tool definitions for multi-turn interactions (e.g., `lookup_contact` for
//! the classifier's contact graph queries). System prompts are Askama templates
//! that include the expected JSON response format.

use askama::Template;
use naked_pineapple_services::claude::Tool;
use serde_json::json;

/// Build the `lookup_contact` tool definition for the contact graph.
///
/// Allows Claude to search the contact graph for people, companies, or domains
/// during classification to identify known senders and their relationships.
#[must_use]
pub fn lookup_contact_tool() -> Tool {
    Tool {
        name: "lookup_contact".to_string(),
        description: "Look up a person, company, or email domain in the contact graph. \
            Use this when you encounter a sender, organization, or domain you want to \
            identify. Returns contact details and their business relationships to \
            Pineapple Skin Co."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Name, email address, or domain to search for."
                }
            }
        }),
        domain: Some("email_triage".to_string()),
        requires_confirmation: false,
    }
}

/// Askama template for the classification system prompt.
#[derive(Template)]
#[template(path = "prompts/classify_email_system.txt")]
struct ClassificationSystemPrompt;

/// System prompt for the email classification step.
///
/// # Panics
///
/// Panics if the Askama template fails to render.
#[must_use]
pub fn classification_system_prompt() -> String {
    ClassificationSystemPrompt
        .render()
        .expect("classification system prompt template")
}

/// Askama template for the response drafting system prompt.
#[derive(Template)]
#[template(path = "prompts/compose_reply_system.txt")]
struct ResponseSystemPrompt;

/// System prompt for the response drafting step.
///
/// # Panics
///
/// Panics if the Askama template fails to render.
#[must_use]
pub fn response_system_prompt() -> String {
    ResponseSystemPrompt
        .render()
        .expect("response system prompt template")
}

/// Askama template for the graph update system prompt.
#[derive(Template)]
#[template(path = "prompts/graph_update_system.txt")]
struct GraphUpdateSystemPrompt;

/// System prompt for the graph update extraction step.
///
/// # Panics
///
/// Panics if the Askama template fails to render.
#[must_use]
pub fn graph_update_system_prompt() -> String {
    GraphUpdateSystemPrompt
        .render()
        .expect("graph update system prompt template")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_contact_tool_has_required_fields() {
        let tool = lookup_contact_tool();
        assert_eq!(tool.name, "lookup_contact");
        assert!(!tool.requires_confirmation);

        let required = tool
            .input_schema
            .get("required")
            .and_then(|v| v.as_array())
            .expect("required array");
        assert!(required.iter().any(|v| v == "query"));
    }

    #[test]
    fn test_classification_system_prompt_renders() {
        let prompt = classification_system_prompt();
        assert!(prompt.contains("Pineapple Skin Co."));
        assert!(prompt.contains("business_vendor"));
        assert!(prompt.contains("lookup_contact"));
        assert!(prompt.contains("\"classification\""));
    }

    #[test]
    fn test_response_system_prompt_renders() {
        let prompt = response_system_prompt();
        assert!(prompt.contains("Pineapple Skin Co."));
        assert!(prompt.contains("\"body_text\""));
    }

    #[test]
    fn test_graph_update_system_prompt_renders() {
        let prompt = graph_update_system_prompt();
        assert!(prompt.contains("Pineapple Skin Co."));
        assert!(prompt.contains("\"contacts\""));
        assert!(prompt.contains("no_update_reason"));
    }
}
