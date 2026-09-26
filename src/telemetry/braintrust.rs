//! Braintrust tracing ingestion for blnk sessions.
//!
//! blnk is a CLI/WebRTC tool with no LLM calls, so instead of the Braintrust
//! SDKs we implement a minimal OTLP/HTTP (JSON) trace exporter that forwards
//! per-session telemetry spans to the Braintrust-hosted OpenTelemetry endpoint.
//!
//! Configuration (environment variables):
//! - `BRAINTRUST_API_KEY` — required; used for `Authorization: Bearer`
//! - `BRAINTRUST_PROJECT_ID` — required; target project (`x-bt-parent` header)
//! - `BRAINTRUST_OTEL_ENDPOINT` — optional; overrides the OTLP endpoint
//!   (defaults to `https://api.braintrust.dev/otel/v1/traces`; use
//!   `https://api-eu.braintrust.dev/otel/v1/traces` for the EU data plane)
//!
//! When either required variable is missing, [`BraintrustExporter::from_env`]
//! returns `Ok(None)` and callers should silently skip export — telemetry must
//! never break a session (fail-open, like the rest of blnk's observability).

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

const DEFAULT_OTEL_ENDPOINT: &str = "https://api.braintrust.dev/otel/v1/traces";
const EXPORT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_SPANS_PER_BATCH: usize = 128;

/// Capability-specific span kinds forwarded to Braintrust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    Session,
    Shell,
    File,
    Proxy,
}

/// A single telemetry span captured from a blnk session.
#[derive(Debug, Clone)]
pub struct TelemetrySpan {
    pub name: String,
    pub kind: SpanKind,
    pub session_id: String,
    pub peer_id: Option<String>,
    pub stream_id: Option<u32>,
    pub started_at_unix: u64,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
}

impl TelemetrySpan {
    pub fn new(name: impl Into<String>, kind: SpanKind, session_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind,
            session_id: session_id.into(),
            peer_id: None,
            stream_id: None,
            started_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default(),
            duration_ms: 0,
            success: true,
            error: None,
        }
    }
}

/// Minimal OTLP/HTTP exporter for the Braintrust-hosted endpoint.
#[derive(Debug, Clone)]
pub struct BraintrustExporter {
    endpoint: String,
    api_key: String,
    project_id: String,
    client: reqwest::Client,
}

impl BraintrustExporter {
    /// Build an exporter from the environment. Returns `Ok(None)` when
    /// telemetry is not configured (missing `BRAINTRUST_API_KEY` or
    /// `BRAINTRUST_PROJECT_ID`).
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let Some(api_key) = std::env::var("BRAINTRUST_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
        else {
            return Ok(None);
        };
        let Some(project_id) = std::env::var("BRAINTRUST_PROJECT_ID")
            .ok()
            .filter(|id| !id.trim().is_empty())
        else {
            tracing::debug!(
                "BRAINTRUST_API_KEY set but BRAINTRUST_PROJECT_ID missing; telemetry disabled"
            );
            return Ok(None);
        };
        let endpoint = std::env::var("BRAINTRUST_OTEL_ENDPOINT")
            .ok()
            .filter(|url| !url.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_OTEL_ENDPOINT.to_owned());

        Ok(Some(Self {
            endpoint,
            api_key,
            project_id,
            client: reqwest::Client::builder()
                .timeout(EXPORT_TIMEOUT)
                .build()?,
        }))
    }

    /// Export a batch of spans, capped at [`MAX_SPANS_PER_BATCH`].
    ///
    /// Export failures are logged and swallowed: telemetry is best-effort and
    /// must never fail a blnk operation.
    pub async fn export(&self, spans: &[TelemetrySpan]) {
        if spans.is_empty() {
            return;
        }
        let batch = spans
            .iter()
            .take(MAX_SPANS_PER_BATCH)
            .map(|span| self.encode_span(span))
            .collect::<Vec<_>>();

        let payload = json!({
            "resourceSpans": [{
                "resource": {
                    "attributes": [{
                        "key": "service.name",
                        "value": { "stringValue": "blnk" }
                    }]
                },
                "scopeSpans": [{
                    "scope": { "name": "blnk.telemetry" },
                    "spans": batch,
                }]
            }]
        });

        let result = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header(
                "x-bt-parent",
                format!("project_id:{}", self.project_id),
            )
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await;

        match result {
            Ok(response) if response.status().is_success() => {
                tracing::debug!(count = spans.len(), "braintrust spans exported");
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                let body = body.chars().take(200).collect::<String>();
                tracing::warn!(status = %status, body = %body, "braintrust export rejected");
            }
            Err(error) => {
                tracing::warn!(error = %error, "braintrust export failed");
            }
        }
    }

    /// Convert a [`TelemetrySpan`] into the OTLP JSON span shape.
    fn encode_span(&self, span: &TelemetrySpan) -> Value {
        let mut attributes = vec![
            string_attr("blnk.session_id", &span.session_id),
            string_attr("blnk.span_kind", span_kind_label(span.kind)),
            bool_attr("blnk.success", span.success),
        ];
        if let Some(peer_id) = &span.peer_id {
            attributes.push(string_attr("blnk.peer_id", peer_id));
        }
        if let Some(stream_id) = span.stream_id {
            attributes.push(int_attr("blnk.stream_id", i64::from(stream_id)));
        }
        if let Some(error) = &span.error {
            attributes.push(string_attr("error.message", error));
        }

        json!({
            "traceId": trace_id(),
            "spanId": span_id(),
            "name": span.name,
            "kind": 1,
            "startTimeUnixNano": span.started_at_unix.saturating_mul(1_000_000_000),
            "endTimeUnixNano": span
                .started_at_unix
                .saturating_add(span.duration_ms)
                .saturating_mul(1_000_000_000),
            "attributes": attributes,
            "status": if span.success {
                json!({ "code": 1 })
            } else {
                json!({ "code": 2, "message": span.error.clone().unwrap_or_default() })
            },
        })
    }
}

fn span_kind_label(kind: SpanKind) -> &'static str {
    match kind {
        SpanKind::Session => "session",
        SpanKind::Shell => "shell",
        SpanKind::File => "file",
        SpanKind::Proxy => "proxy",
    }
}

fn string_attr(key: &str, value: &str) -> Value {
    json!({ "key": key, "value": { "stringValue": value } })
}

fn bool_attr(key: &str, value: bool) -> Value {
    json!({ "key": key, "value": { "boolValue": value } })
}

fn int_attr(key: &str, value: i64) -> Value {
    json!({ "key": key, "value": { "intValue": value } })
}

fn trace_id() -> String {
    // OTLP trace ids are 16 bytes; a UUID hex string is the same width.
    Uuid::new_v4().simple().to_string()
}

fn span_id() -> String {
    // OTLP span ids are 8 bytes; take the first 16 hex chars of a UUID.
    let hex = Uuid::new_v4().simple().to_string();
    hex[..16].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_span() -> TelemetrySpan {
        let mut span = TelemetrySpan::new("session.ready", SpanKind::Session, "session-1");
        span.peer_id = Some("peer-9".to_owned());
        span.stream_id = Some(3);
        span.duration_ms = 42;
        span
    }

    #[test]
    fn from_env_is_disabled_without_configuration() {
        // Tests run without Braintrust credentials; the exporter must be
        // disabled rather than erroring so telemetry stays fail-open.
        if std::env::var("BRAINTRUST_API_KEY").is_ok() {
            return;
        }
        let exporter = BraintrustExporter::from_env().expect("disabled export must not error");
        assert!(exporter.is_none());
    }

    #[test]
    fn encode_span_maps_all_fields_to_otlp_shape() {
        let exporter = BraintrustExporter {
            endpoint: "https://otel.test/v1/traces".to_owned(),
            api_key: "key".to_owned(),
            project_id: "project".to_owned(),
            client: reqwest::Client::new(),
        };
        let span = sample_span();
        let value = exporter.encode_span(&span);

        assert_eq!(value["name"], "session.ready");
        assert_eq!(value["kind"], 1);
        assert_eq!(value["status"]["code"], 1);
        assert_eq!(value["traceId"].as_str().unwrap().len(), 32);
        assert_eq!(value["spanId"].as_str().unwrap().len(), 16);
        let expected_start = span.started_at_unix.saturating_mul(1_000_000_000);
        let expected_end = span
            .started_at_unix
            .saturating_add(span.duration_ms)
            .saturating_mul(1_000_000_000);
        assert_eq!(value["startTimeUnixNano"], expected_start);
        assert_eq!(value["endTimeUnixNano"], expected_end);

        let attrs = value["attributes"].as_array().unwrap();
        let find = |key: &str| {
            attrs
                .iter()
                .find(|attr| attr["key"] == key)
                .expect("attribute must exist")
                .clone()
        };
        assert_eq!(find("blnk.session_id")["value"]["stringValue"], "session-1");
        assert_eq!(find("blnk.span_kind")["value"]["stringValue"], "session");
        assert_eq!(find("blnk.peer_id")["value"]["stringValue"], "peer-9");
        assert_eq!(find("blnk.stream_id")["value"]["intValue"], 3);
        assert_eq!(find("blnk.success")["value"]["boolValue"], true);
    }

    #[test]
    fn encode_span_marks_failed_status_and_error_attribute() {
        let exporter = BraintrustExporter {
            endpoint: "https://otel.test/v1/traces".to_owned(),
            api_key: "key".to_owned(),
            project_id: "project".to_owned(),
            client: reqwest::Client::new(),
        };
        let mut span = sample_span();
        span.success = false;
        span.error = Some("pin rejected".to_owned());

        let value = exporter.encode_span(&span);
        assert_eq!(value["status"]["code"], 2);
        assert_eq!(value["status"]["message"], "pin rejected");
        let attrs = value["attributes"].as_array().unwrap();
        let error_attr = attrs
            .iter()
            .find(|attr| attr["key"] == "error.message")
            .expect("error attribute must exist");
        assert_eq!(error_attr["value"]["stringValue"], "pin rejected");
    }

    #[test]
    fn span_ids_and_trace_ids_are_hex() {
        let trace = trace_id();
        assert!(trace.chars().all(|c| c.is_ascii_hexdigit()));
        let span = span_id();
        assert!(span.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
