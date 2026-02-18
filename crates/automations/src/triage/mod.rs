//! Email triage pipeline.
//!
//! Orchestrates the two-step AI triage process:
//! 1. **Classification** — Claude classifies the email and extracts entities
//! 2. **Routing** — Based on classification, the email is archived, queued for
//!    Slack review, or routed to Klaviyo Helpdesk
//!
//! The library exposes `classifier`, `graph_updater`, `tools`, and `types`
//! for use by both the automations binary and the admin panel. The `router`,
//! `responder`, and `pipeline` modules are binary-only (declared in `main.rs`).

pub mod classifier;
pub mod graph_updater;
pub mod tools;
pub mod types;

/// Truncate a string to `max_len` bytes with an "...(truncated)" suffix.
///
/// Finds the nearest valid UTF-8 char boundary at or before `max_len` to
/// avoid panics on multi-byte characters.
#[must_use]
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...(truncated)", &s[..end])
}

/// Extract a JSON value from an LLM text response.
///
/// Tries, in order: the full string as JSON, a fenced ` ```json ` block,
/// and finally the first `{` to the last `}`.
///
/// # Errors
///
/// Returns `ClaudeError::Parse` if no valid JSON can be extracted.
pub fn extract_json(
    text: &str,
) -> Result<serde_json::Value, naked_pineapple_services::claude::ClaudeError> {
    use naked_pineapple_services::claude::ClaudeError;

    let trimmed = text.trim();

    // Try the whole string
    if let Ok(v) = serde_json::from_str(trimmed) {
        return Ok(v);
    }

    // Try ```json ... ``` fenced block
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            let json_str = trimmed[json_start..json_start + end].trim();
            if let Ok(v) = serde_json::from_str(json_str) {
                return Ok(v);
            }
        }
    }

    // Try first { to last }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}'))
        && start < end
        && let Ok(v) = serde_json::from_str(&trimmed[start..=end])
    {
        return Ok(v);
    }

    Err(ClaudeError::Parse(
        "could not extract JSON from response".to_string(),
    ))
}
