//! Structured logging configuration for the automations service.
//!
//! Reuses the same patterns as the admin binary:
//! - JSON format on Fly.io with service metadata and OTel-style field names
//! - Human-readable format for local development
//! - Sentry integration for error tracking

use std::io;

use sentry::integrations::tracing as sentry_tracing;
use serde_json::{Map, Value};
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

/// Field names normalized to `error.message` (OpenTelemetry convention).
const ERROR_FIELD_ALIASES: &[&str] = &["error", "err"];

/// Service metadata from environment, injected into every log line.
#[derive(Clone)]
pub struct ServiceMetadata {
    pub service_name: &'static str,
    pub service_version: &'static str,
    pub host_name: String,
    pub cloud_region: Option<String>,
    pub cloud_app: Option<String>,
}

impl ServiceMetadata {
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

    pub const fn is_fly(&self) -> bool {
        self.cloud_app.is_some()
    }
}

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

fn normalize_error_fields(obj: &mut Map<String, Value>) {
    for alias in ERROR_FIELD_ALIASES {
        if let Some(value) = obj.remove(*alias) {
            obj.entry("error.message".to_string()).or_insert(value);
        }
    }
}

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

fn sentry_event_filter(metadata: &tracing::Metadata<'_>) -> sentry_tracing::EventFilter {
    match *metadata.level() {
        tracing::Level::ERROR | tracing::Level::WARN => sentry_tracing::EventFilter::Event,
        tracing::Level::INFO | tracing::Level::DEBUG => sentry_tracing::EventFilter::Breadcrumb,
        _ => sentry_tracing::EventFilter::Ignore,
    }
}

/// Initialize the tracing subscriber.
///
/// On Fly.io: JSON format with service metadata.
/// Locally: Human-readable text format with colors.
pub fn init_tracing(metadata: &ServiceMetadata) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "naked_pineapple_automations=info,tower_http=debug".into());

    if metadata.is_fly() {
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
        let text_layer = fmt::layer().pretty();

        tracing_subscriber::registry()
            .with(env_filter)
            .with(text_layer)
            .with(sentry_tracing::layer().event_filter(sentry_event_filter))
            .init();
    }
}
