//! Handlers for `/.well-known/` endpoints ([RFC 8615]).
//!
//! Includes `WebAuthn` Related Origin Requests (ROR) for multi-domain passkey support.
//!
//! [RFC 8615]: https://www.rfc-editor.org/rfc/rfc8615

use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use serde::Serialize;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/webauthn", get(webauthn_related_origins))
        .route("/security.txt", get(security_txt))
}

/// `WebAuthn` Related Origin Requests response.
#[derive(Serialize)]
struct RelatedOrigins {
    origins: Vec<String>,
}

/// Serve the `WebAuthn` Related Origin Requests JSON.
///
/// Per the W3C spec, the RP ID origin is implicitly allowed and must NOT
/// appear in the list. Only non-primary origins are included.
async fn webauthn_related_origins(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config();
    let origins: Vec<String> = config
        .hosts
        .iter()
        .filter(|h| *h != &config.primary_host)
        .map(|h| config.origin_for(h))
        .collect();

    (StatusCode::OK, Json(RelatedOrigins { origins }))
}

/// Security contact information ([RFC 9116]).
///
/// [RFC 9116]: https://www.rfc-editor.org/rfc/rfc9116
async fn security_txt() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "Contact: mailto:security@pineappleskinco.com\n\
         Expires: 2027-02-05T00:00:00z\n\
         Preferred-Languages: en\n",
    )
}
