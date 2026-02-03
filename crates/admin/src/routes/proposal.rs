//! Proposal page route handler.
//!
//! Renders a branded, scroll-driven proposal page at `/proposal`.
//! No authentication required — designed for prospective clients.

use askama::Template;
use axum::{Router, response::Html, routing::get};

use crate::filters;
use crate::state::AppState;

/// Proposal page template.
#[derive(Template)]
#[template(path = "proposal.html")]
struct ProposalPageTemplate;

/// Build the proposal router.
pub fn router() -> Router<AppState> {
    Router::new().route("/proposal", get(proposal_page))
}

/// Render the proposal page.
///
/// GET /proposal
async fn proposal_page() -> Html<String> {
    Html(
        ProposalPageTemplate
            .render()
            .unwrap_or_else(|_| String::from("Error rendering template")),
    )
}
