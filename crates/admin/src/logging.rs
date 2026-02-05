//! Structured logging configuration for production observability.
//!
//! This module configures tracing output with:
//! - JSON format on Fly.io with service metadata and OTel-style field names
//! - Human-readable format for local development
//! - Sentry integration for error tracking
//!
//! # Field Naming Convention
//!
//! We use OpenTelemetry semantic conventions for field names:
//! - `service.name` - Application identifier
//! - `service.version` - Application version from Cargo
//! - `host.name` - Fly.io allocation ID or hostname
//! - `cloud.region` - Fly.io region
//! - `http.method` - HTTP request method
//! - `http.route` - URL path
//! - `http.status_code` - Response status
//! - `http.request.duration_ms` - Request latency

use std::io;

use sentry::integrations::tracing as sentry_tracing;
use serde_json::{Map, Value};
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

/// Field names that should be normalized to `error.message` (OpenTelemetry convention).
/// These come from various third-party crates that use inconsistent naming.
const ERROR_FIELD_ALIASES: &[&str] = &["error", "err", "classification"];

/// Service metadata from environment, injected into every log line.
#[derive(Clone)]
pub struct ServiceMetadata {
    /// Service name (e.g., "storefront", "admin")
    pub service_name: &'static str,
    /// Service version from Cargo.toml
    pub service_version: &'static str,
    /// Fly.io allocation ID or local hostname
    pub host_name: String,
    /// Fly.io region (e.g., "sjc", "iad")
    pub cloud_region: Option<String>,
    /// Fly.io app name
    pub cloud_app: Option<String>,
}

impl ServiceMetadata {
    /// Create metadata from environment variables.
    ///
    /// On Fly.io, reads `FLY_ALLOC_ID`, `FLY_REGION`, and `FLY_APP_NAME`.
    /// Locally, uses "local" as the host identifier.
    pub fn from_env(service_name: &'static str) -> Self {
        let host_name = std::env::var("FLY_ALLOC_ID").unwrap_or_else(|_| "local".to_string());

        Self {
            service_name,
            service_version: env!("CARGO_PKG_VERSION"),
            host_name,
            cloud_region: std::env::var("FLY_REGION").ok(),
            cloud_app: std::env::var("FLY_APP_NAME").ok(),
        }
    }

    /// Returns true if running on Fly.io.
    pub const fn is_fly(&self) -> bool {
        self.cloud_app.is_some()
    }
}

/// A writer that transforms JSON log lines for production observability.
///
/// For each JSON log line, this writer:
/// 1. Normalizes error field names to OpenTelemetry conventions (`error.message`)
/// 2. Injects service metadata fields
struct MetadataInjectingWriter<W> {
    inner: W,
    metadata: ServiceMetadata,
}

impl<W: io::Write> io::Write for MetadataInjectingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let line = String::from_utf8_lossy(buf);

        if line.starts_with('{') {
            match serde_json::from_str::<Value>(&line) {
                Ok(mut json) => {
                    if let Some(obj) = json.as_object_mut() {
                        normalize_error_fields(obj);
                        inject_service_metadata(obj, &self.metadata);
                    }
                    let mut output =
                        serde_json::to_string(&json).unwrap_or_else(|_| line.into_owned());
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                    self.inner.write_all(output.as_bytes())?;
                }
                Err(_) => {
                    // Not valid JSON, pass through unchanged
                    self.inner.write_all(buf)?;
                }
            }
        } else {
            self.inner.write_all(buf)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Normalize error field aliases to OTel-compliant `error.message`.
fn normalize_error_fields(obj: &mut Map<String, Value>) {
    for alias in ERROR_FIELD_ALIASES {
        if let Some(value) = obj.remove(*alias) {
            // Only set if not already present (first wins)
            obj.entry("error.message".to_string()).or_insert(value);
        }
    }
}

/// Inject service metadata fields into the JSON object.
fn inject_service_metadata(obj: &mut Map<String, Value>, metadata: &ServiceMetadata) {
    obj.insert(
        "service.name".to_string(),
        Value::String(metadata.service_name.to_string()),
    );
    obj.insert(
        "service.version".to_string(),
        Value::String(metadata.service_version.to_string()),
    );
    obj.insert(
        "host.name".to_string(),
        Value::String(metadata.host_name.clone()),
    );
    if let Some(region) = &metadata.cloud_region {
        obj.insert("cloud.region".to_string(), Value::String(region.clone()));
    }
    if let Some(app) = &metadata.cloud_app {
        obj.insert("cloud.app".to_string(), Value::String(app.clone()));
    }
}

/// A `MakeWriter` that wraps another `MakeWriter` and transforms JSON output.
#[derive(Clone)]
struct MetadataInjectingMakeWriter<M> {
    inner: M,
    metadata: ServiceMetadata,
}

impl<'a, M> MakeWriter<'a> for MetadataInjectingMakeWriter<M>
where
    M: MakeWriter<'a>,
{
    type Writer = MetadataInjectingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        MetadataInjectingWriter {
            inner: self.inner.make_writer(),
            metadata: self.metadata.clone(),
        }
    }
}

/// Filter tracing events to Sentry event types.
fn sentry_event_filter(metadata: &tracing::Metadata<'_>) -> sentry_tracing::EventFilter {
    match *metadata.level() {
        tracing::Level::ERROR | tracing::Level::WARN => sentry_tracing::EventFilter::Event,
        tracing::Level::INFO | tracing::Level::DEBUG => sentry_tracing::EventFilter::Breadcrumb,
        _ => sentry_tracing::EventFilter::Ignore,
    }
}

/// Initialize the tracing subscriber with appropriate formatting.
///
/// On Fly.io (when `FLY_APP_NAME` is set):
/// - Uses JSON format with flattened events
/// - Injects service metadata into every log line
///
/// Locally:
/// - Uses human-readable text format with colors
///
/// Both modes include Sentry integration for error tracking.
pub fn init_tracing(metadata: &ServiceMetadata) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "naked_pineapple_admin=info,tower_http=debug".into());

    if metadata.is_fly() {
        // JSON format for production with metadata injection and field normalization
        let make_writer = MetadataInjectingMakeWriter {
            inner: io::stdout,
            metadata: metadata.clone(),
        };

        let json_layer = fmt::layer()
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_writer(make_writer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(json_layer)
            .with(sentry_tracing::layer().event_filter(sentry_event_filter))
            .init();
    } else {
        // Pretty text format for local development
        let text_layer = fmt::layer().pretty();

        tracing_subscriber::registry()
            .with(env_filter)
            .with(text_layer)
            .with(sentry_tracing::layer().event_filter(sentry_event_filter))
            .init();
    }
}

/// Create a span for HTTP request tracing with OTel-style field names.
///
/// This should be used with `tower_http::trace::TraceLayer::make_span_with()`.
///
/// Fields included:
/// - `http.method` - Request method (GET, POST, etc.)
/// - `http.route` - Request URI path
/// - `http.user_agent` - User-Agent header value
/// - `http.client_ip` - Client IP from X-Forwarded-For or connection
/// - `request_id` - Unique request identifier (populated by middleware)
/// - `http.status_code` - Response status (populated on response)
/// - `http.request.duration_ms` - Request duration (populated on response)
pub fn make_http_span<B>(request: &axum::http::Request<B>) -> tracing::Span {
    let user_agent = request
        .headers()
        .get(axum::http::header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");

    // Get client IP from X-Forwarded-For (set by Fly.io/Tailscale proxy) or fall back to "-"
    let client_ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map_or("-", str::trim);

    tracing::info_span!(
        "http_request",
        http.method = %request.method(),
        http.route = %request.uri().path(),
        http.user_agent = %user_agent,
        http.client_ip = %client_ip,
        request_id = tracing::field::Empty,
        http.status_code = tracing::field::Empty,
        http.request.duration_ms = tracing::field::Empty,
    )
}

/// Record response information on the current span.
///
/// This should be used with `tower_http::trace::TraceLayer::on_response()`.
pub fn on_http_response<B>(
    response: &axum::http::Response<B>,
    latency: std::time::Duration,
    span: &tracing::Span,
) {
    span.record("http.status_code", response.status().as_u16());
    span.record(
        "http.request.duration_ms",
        u64::try_from(latency.as_millis()).unwrap_or(u64::MAX),
    );
}

/// Record failure information with OpenTelemetry-style field names.
///
/// This should be used with `tower_http::trace::TraceLayer::on_failure()`.
/// Prevents tower-http from emitting non-standard `classification` field.
#[expect(
    clippy::needless_pass_by_value,
    reason = "OnFailure trait requires failure to be passed by value"
)]
pub fn on_http_failure(
    failure: tower_http::classify::ServerErrorsFailureClass,
    _latency: std::time::Duration,
    _span: &tracing::Span,
) {
    use tower_http::classify::ServerErrorsFailureClass;

    let error_message = match &failure {
        ServerErrorsFailureClass::StatusCode(code) => format!("HTTP {}", code.as_u16()),
        ServerErrorsFailureClass::Error(msg) => msg.clone(),
    };

    tracing::error!("error.message" = %error_message, "Request failed");
}
