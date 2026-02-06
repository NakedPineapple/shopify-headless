//! Handlers for `/.well-known/` endpoints ([RFC 8615]).
//!
//! [RFC 8615]: https://www.rfc-editor.org/rfc/rfc8615

use axum::{
    Router,
    http::{StatusCode, header},
    response::{IntoResponse, Redirect},
    routing::get,
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/gpc.json", get(gpc_json))
        .route("/security.txt", get(security_txt))
        .route("/change-password", get(change_password))
}

/// GPC support resource ([Global Privacy Control specification]).
///
/// Signals to browsers and extensions that this site honors the
/// `Sec-GPC` opt-out preference signal.
///
/// [Global Privacy Control specification]: https://globalprivacycontrol.github.io/gpc-spec/
async fn gpc_json() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"gpc":true,"lastUpdate":"2026-02-05"}"#,
    )
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

/// Password change well-known URL ([W3C specification]).
///
/// Browsers and password managers redirect users here to change their password.
///
/// [W3C specification]: https://w3c.github.io/webappsec-change-password-url/
async fn change_password() -> Redirect {
    Redirect::to("/account/change-password")
}
