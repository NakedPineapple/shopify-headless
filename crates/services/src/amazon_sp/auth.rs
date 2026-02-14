//! Amazon SP-API authentication: LWA token exchange + AWS `SigV4` signing.

use std::time::SystemTime;

use aws_credential_types::Credentials;
use aws_sigv4::http_request::{
    PayloadChecksumKind, SignableBody, SignableRequest, SignatureLocation, SigningSettings,
    sign as sigv4_sign,
};
use aws_sigv4::sign::v4;
use secrecy::ExposeSecret;

use super::AmazonSpError;
use super::types::{AmazonCredentials, LwaToken, LwaTokenResponse};

/// LWA token endpoint.
const LWA_TOKEN_URL: &str = "https://api.amazon.com/auth/o2/token";

/// AWS region for SP-API (North America).
const SP_API_REGION: &str = "us-east-1";

/// AWS service name for SP-API.
const SP_API_SERVICE: &str = "execute-api";

/// Exchange an LWA refresh token for an access token.
///
/// # Errors
///
/// Returns `AmazonSpError::TokenExchange` if the request fails.
pub async fn exchange_refresh_token(
    client: &reqwest::Client,
    credentials: &AmazonCredentials,
) -> Result<LwaToken, AmazonSpError> {
    let params = [
        ("grant_type", "refresh_token"),
        (
            "refresh_token",
            credentials.lwa_refresh_token.expose_secret(),
        ),
        ("client_id", &credentials.lwa_client_id),
        (
            "client_secret",
            credentials.lwa_client_secret.expose_secret(),
        ),
    ];

    let response = client
        .post(LWA_TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| AmazonSpError::TokenExchange(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(AmazonSpError::TokenExchange(format!("{status}: {body}")));
    }

    let token_response: LwaTokenResponse = response
        .json()
        .await
        .map_err(|e| AmazonSpError::TokenExchange(format!("Failed to parse response: {e}")))?;

    let now = chrono::Utc::now().timestamp();
    Ok(LwaToken {
        access_token: token_response.access_token,
        expires_at: now + token_response.expires_in,
    })
}

/// Sign a request with AWS Signature Version 4.
///
/// Mutates the provided `reqwest::Request` by adding authorization headers.
///
/// # Errors
///
/// Returns `AmazonSpError::Signing` if signing fails.
pub fn sign_request(
    request: &mut reqwest::Request,
    credentials: &AmazonCredentials,
) -> Result<(), AmazonSpError> {
    let aws_creds = Credentials::new(
        &credentials.aws_access_key_id,
        credentials.aws_secret_access_key.expose_secret(),
        None,
        None,
        "amazon-sp-api",
    );

    let mut settings = SigningSettings::default();
    settings.signature_location = SignatureLocation::Headers;
    settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;

    let identity = aws_creds.into();
    let signing_params = v4::SigningParams::builder()
        .identity(&identity)
        .region(SP_API_REGION)
        .name(SP_API_SERVICE)
        .time(SystemTime::now())
        .settings(settings)
        .build()
        .map_err(|e| AmazonSpError::Signing(e.to_string()))?;

    let method = request.method().as_str().to_string();
    let uri = request.url().to_string();
    let headers: Vec<(String, String)> = request
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect();

    let body = request.body().and_then(|b| b.as_bytes()).unwrap_or(b"");

    let signable_request = SignableRequest::new(
        &method,
        &uri,
        headers.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        SignableBody::Bytes(body),
    )
    .map_err(|e| AmazonSpError::Signing(e.to_string()))?;

    let (signing_instructions, _signature) = sigv4_sign(signable_request, &signing_params.into())
        .map_err(|e| AmazonSpError::Signing(e.to_string()))?
        .into_parts();

    // Build an http::Request to apply the signing instructions to
    let mut http_req = http::Request::builder()
        .method(method.as_str())
        .uri(&uri)
        .body(())
        .map_err(|e| AmazonSpError::Signing(e.to_string()))?;

    signing_instructions.apply_to_request_http1x(&mut http_req);

    // Copy the new signing headers back to the reqwest request
    for (name, value) in http_req.headers() {
        request.headers_mut().insert(name.clone(), value.clone());
    }

    Ok(())
}
