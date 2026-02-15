//! TikTok Shop API request signing (HMAC-SHA256).
//!
//! Every TikTok Shop API request must include a `sign` query parameter computed
//! as `HMAC-SHA256(app_secret, path + sorted_params + body)`.

use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;
use sha2::Sha256;

use super::TikTokShopError;

type HmacSha256 = Hmac<Sha256>;

/// Generate HMAC-SHA256 signature for a TikTok Shop API request.
///
/// Signature format: `HMAC-SHA256(app_secret, path + key1value1key2value2... + body)`
///
/// # Errors
///
/// Returns `TikTokShopError::Signing` if the HMAC key is invalid.
pub fn sign_request(
    app_secret: &secrecy::SecretString,
    path: &str,
    params: &[(String, String)],
    body: Option<&str>,
) -> Result<String, TikTokShopError> {
    // Sort params alphabetically by key.
    let mut sorted_params = params.to_vec();
    sorted_params.sort_by(|a, b| a.0.cmp(&b.0));

    // Build the string to sign: path + key1value1key2value2... + body
    let mut sign_string = path.to_string();
    for (key, value) in &sorted_params {
        sign_string.push_str(key);
        sign_string.push_str(value);
    }
    if let Some(b) = body {
        sign_string.push_str(b);
    }

    let mut mac = HmacSha256::new_from_slice(app_secret.expose_secret().as_bytes())
        .map_err(|e| TikTokShopError::Signing(e.to_string()))?;
    mac.update(sign_string.as_bytes());
    let result = mac.finalize();

    Ok(hex::encode(result.into_bytes()))
}
