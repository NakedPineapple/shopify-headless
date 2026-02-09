//! Webhook signature verification functions.
//!
//! Each webhook source uses its own authentication mechanism. Signature
//! verification always runs on the raw request body before any parsing
//! or database access.

use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Verify an HMAC-SHA256 signature where the expected value is base64-encoded.
///
/// Used by Shopify (`X-Shopify-Hmac-Sha256`).
pub fn verify_hmac_sha256_base64(secret: &[u8], body: &[u8], signature: &str) -> bool {
    let Ok(mut mac) = HmacSha256::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);

    let Ok(expected) = base64::engine::general_purpose::STANDARD.decode(signature) else {
        return false;
    };
    mac.verify_slice(&expected).is_ok()
}

/// Verify an HMAC-SHA256 signature where the expected value is hex-encoded
/// with a `sha256=` prefix.
///
/// Used by GitHub (`X-Hub-Signature-256`) and Sentry (`Sentry-Hook-Signature`).
pub fn verify_hmac_sha256_hex(secret: &[u8], body: &[u8], signature: &str) -> bool {
    let hex_sig = signature.strip_prefix("sha256=").unwrap_or(signature);

    let Ok(mut mac) = HmacSha256::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);

    let Ok(expected) = hex::decode(hex_sig) else {
        return false;
    };
    mac.verify_slice(&expected).is_ok()
}

/// Verify a bearer token from the `Authorization` header.
///
/// Uses HMAC-based comparison to achieve constant-time equality checking
/// without adding a direct `subtle` dependency.
///
/// Used by Fly.io and Better Stack.
pub fn verify_bearer_token(expected: &str, authorization: &str) -> bool {
    let token = authorization
        .strip_prefix("Bearer ")
        .unwrap_or(authorization);

    // Use HMAC to achieve constant-time comparison: HMAC(key=expected, msg=token)
    // matches HMAC(key=expected, msg=expected) only when token == expected.
    let Ok(mut mac) = HmacSha256::new_from_slice(expected.as_bytes()) else {
        return false;
    };
    mac.update(token.as_bytes());
    let got = mac.finalize().into_bytes();

    let Ok(mut mac2) = HmacSha256::new_from_slice(expected.as_bytes()) else {
        return false;
    };
    mac2.update(expected.as_bytes());
    let want = mac2.finalize().into_bytes();

    got == want
}

/// Decode a hex string into bytes. Returns `None` on invalid hex.
mod hex {
    pub fn decode(s: &str) -> Result<Vec<u8>, ()> {
        if !s.len().is_multiple_of(2) {
            return Err(());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_sha256_base64_valid() {
        let secret = b"test-secret";
        let body = b"test body";

        let mut mac = HmacSha256::new_from_slice(secret).unwrap();
        mac.update(body);
        let sig = base64::engine::general_purpose::STANDARD
            .encode(mac.finalize().into_bytes());

        assert!(verify_hmac_sha256_base64(secret, body, &sig));
    }

    #[test]
    fn test_hmac_sha256_base64_invalid() {
        let secret = b"test-secret";
        let body = b"test body";
        let bad_sig = base64::engine::general_purpose::STANDARD.encode(b"bad");

        assert!(!verify_hmac_sha256_base64(secret, body, &bad_sig));
    }

    #[test]
    fn test_hmac_sha256_hex_valid() {
        let secret = b"test-secret";
        let body = b"test body";

        let mut mac = HmacSha256::new_from_slice(secret).unwrap();
        mac.update(body);
        let result = mac.finalize().into_bytes();
        let sig = format!(
            "sha256={}",
            result.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );

        assert!(verify_hmac_sha256_hex(secret, body, &sig));
    }

    #[test]
    fn test_hmac_sha256_hex_invalid() {
        assert!(!verify_hmac_sha256_hex(b"secret", b"body", "sha256=0000"));
    }

    #[test]
    fn test_bearer_token_valid() {
        assert!(verify_bearer_token("my-token", "Bearer my-token"));
    }

    #[test]
    fn test_bearer_token_invalid() {
        assert!(!verify_bearer_token("my-token", "Bearer wrong-token"));
    }

    #[test]
    fn test_bearer_token_raw() {
        assert!(verify_bearer_token("my-token", "my-token"));
    }
}
