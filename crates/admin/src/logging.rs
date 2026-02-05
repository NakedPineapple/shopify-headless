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
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

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

    /// Format as JSON prefix for log line injection.
    fn as_json_prefix(&self) -> String {
        let mut parts = vec![
            format!(r#""service.name":"{}""#, self.service_name),
            format!(r#""service.version":"{}""#, self.service_version),
            format!(r#""host.name":"{}""#, self.host_name),
        ];

        if let Some(region) = &self.cloud_region {
            parts.push(format!(r#""cloud.region":"{region}""#));
        }

        if let Some(app) = &self.cloud_app {
            parts.push(format!(r#""cloud.app":"{app}""#));
        }

        parts.join(",")
    }
}

/// A writer that injects service metadata into JSON log lines.
///
/// Each line is expected to be a JSON object. This writer prepends
/// the service metadata fields to the object.
struct MetadataInjectingWriter<W> {
    inner: W,
    metadata_prefix: String,
}

impl<W: io::Write> io::Write for MetadataInjectingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Convert to string to manipulate JSON
        let line = String::from_utf8_lossy(buf);

        // If it starts with '{', inject our metadata after the opening brace
        if let Some(rest) = line.strip_prefix('{') {
            let injected = format!("{{{},{rest}", self.metadata_prefix);
            // Write the injected content but return original length
            // to satisfy the Write contract
            self.inner.write_all(injected.as_bytes())?;
        } else {
            // Not JSON, pass through unchanged
            self.inner.write_all(buf)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// A `MakeWriter` that wraps another `MakeWriter` and injects metadata.
#[derive(Clone)]
struct MetadataInjectingMakeWriter<M> {
    inner: M,
    metadata_prefix: String,
}

impl<'a, M> MakeWriter<'a> for MetadataInjectingMakeWriter<M>
where
    M: MakeWriter<'a>,
{
    type Writer = MetadataInjectingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        MetadataInjectingWriter {
            inner: self.inner.make_writer(),
            metadata_prefix: self.metadata_prefix.clone(),
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
        // JSON format for production with service metadata injection
        let make_writer = MetadataInjectingMakeWriter {
            inner: io::stdout,
            metadata_prefix: metadata.as_json_prefix(),
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
