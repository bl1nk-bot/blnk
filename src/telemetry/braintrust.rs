//! Braintrust tracing ingestion for blnk sessions.
//!
//! blnk is a CLI/WebRTC tool with no LLM calls, so instead of the Braintrust
//! SDKs we implement a minimal OTLP/HTTP (JSON) trace exporter that forwards
//! per-session telemetry spans to the Braintrust-hosted OpenTelemetry endpoint.
//!
//! Span model: one session-level span (`session.serve`, `session.connect`,
//! `session.copy`) per CLI run, with stream-level child spans (`stream.shell`,
//! `stream.file`, `stream.proxy`) nested under it via OTLP `traceId` +
//! `parentSpanId`. Child spans are recorded where streams actually execute
//! (the server dispatcher and the client shell/file ops) and exported together
//! with their parent as a single batch once the command outcome is known.
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

use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
    /// OTLP trace id (32 hex chars). Assigned for session spans; child spans
    /// inherit it so parent and children join into one trace.
    pub trace_id: Option<String>,
    /// OTLP span id (16 hex chars). Assigned for session and child spans so
    /// children can reference their parent.
    pub span_id: Option<String>,
    /// Span id of the parent span, when this span is nested.
    pub parent_span_id: Option<String>,
    /// Monotonic timer captured at creation for millisecond durations.
    pub started: Instant,
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
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            started: Instant::now(),
        }
    }

    /// A session-level root span: owns its trace and span ids.
    pub fn session_span(
        name: impl Into<String>,
        kind: SpanKind,
        session_id: impl Into<String>,
    ) -> Self {
        let mut span = Self::new(name, kind, session_id);
        span.trace_id = Some(trace_id());
        span.span_id = Some(span_id());
        span
    }

    /// A stream-level child span nested under a session span: inherits the
    /// parent's trace id and links via `parentSpanId`.
    pub fn child_span(parent: &Self, name: &str, kind: SpanKind, stream_id: u32) -> Self {
        let session_id = parent.session_id.clone();
        let mut span = Self::new(name, kind, session_id);
        span.trace_id = Some(parent.trace_id.clone().unwrap_or_else(trace_id));
        span.parent_span_id = parent.span_id.clone();
        span.span_id = Some(span_id());
        span.stream_id = Some(stream_id);
        span
    }
}

/// Collects stream-level child spans under an optional session parent.
///
/// The CLI builds one collector per session: the parent is the session span
/// (`session.serve` / `session.connect` / `session.copy`) and every shell,
/// file, or proxy stream executed inside the session records a child span.
/// When telemetry is disabled the collector stays parent-less and every
/// operation becomes a no-op.
#[derive(Debug, Default)]
pub struct SpanCollector {
    parent: Option<TelemetrySpan>,
    children: Vec<TelemetrySpan>,
}

impl SpanCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable the session-level parent span; it owns its trace/span ids so
    /// stream children can nest under it.
    pub fn enable_parent(
        &mut self,
        name: impl Into<String>,
        kind: SpanKind,
        session_id: impl Into<String>,
    ) {
        self.parent = Some(TelemetrySpan::session_span(name, kind, session_id));
    }

    /// Start a stream-level child span under the session parent. Returns
    /// `None` when telemetry is disabled (no parent), making the caller's
    /// recording work a no-op.
    pub fn start_child(&self, name: &str, kind: SpanKind, stream_id: u32) -> Option<TelemetrySpan> {
        self.parent
            .as_ref()
            .map(|parent| TelemetrySpan::child_span(parent, name, kind, stream_id))
    }

    /// Finalize a child span started by [`start_child`], recording duration
    /// and outcome. No-op when telemetry is disabled.
    pub fn finish_child(
        &mut self,
        span: Option<TelemetrySpan>,
        success: bool,
        error: Option<String>,
    ) {
        if let Some(mut span) = span {
            span.duration_ms = span.started.elapsed().as_millis().min(u64::MAX as u128) as u64;
            span.success = success;
            span.error = error.filter(|message| !message.is_empty());
            self.children.push(span);
        }
    }

    /// Finalize the session parent with the command outcome (duration in
    /// milliseconds measured by the caller, success flag, and error message).
    pub fn finish_parent(&mut self, duration_ms: u64, success: bool, error: Option<String>) {
        if let Some(parent) = &mut self.parent {
            parent.duration_ms = duration_ms;
            parent.success = success;
            parent.error = error.filter(|message| !message.is_empty());
        }
    }

    /// Attach the peer/client id to the session parent before export.
    pub fn set_parent_peer(&mut self, peer_id: impl Into<String>) {
        if let Some(parent) = &mut self.parent {
            parent.peer_id = Some(peer_id.into());
        }
    }

    /// All spans for export: parent first, then children in completion order.
    pub fn into_spans(self) -> Vec<TelemetrySpan> {
        let mut spans = Vec::with_capacity(self.children.len() + 1);
        if let Some(parent) = self.parent {
            spans.push(parent);
        }
        spans.extend(self.children);
        spans
    }
}

/// Minimal OTLP/HTTP exporter for the Braintrust-hosted endpoint.
#[derive(Clone)]
pub struct BraintrustExporter {
    endpoint: String,
    api_key: String,
    project_id: String,
    client: reqwest::Client,
}

impl std::fmt::Debug for BraintrustExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BraintrustExporter")
            .field("endpoint", &self.endpoint)
            .field("api_key", &"<redacted>")
            .field("project_id", &self.project_id)
            .field("client", &self.client)
            .finish()
    }
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
            client: reqwest::Client::builder().timeout(EXPORT_TIMEOUT).build()?,
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
            .header("x-bt-parent", format!("project_id:{}", self.project_id))
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

        let mut value = json!({
            "traceId": span.trace_id.clone().unwrap_or_else(trace_id),
            "spanId": span.span_id.clone().unwrap_or_else(span_id),
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
        });
        if let Some(parent) = &span.parent_span_id {
            value["parentSpanId"] = json!(parent);
        }
        value
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

    fn test_exporter() -> BraintrustExporter {
        BraintrustExporter {
            endpoint: "https://otel.test/v1/traces".to_owned(),
            api_key: "key".to_owned(),
            project_id: "project".to_owned(),
            client: reqwest::Client::new(),
        }
    }

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
        let exporter = test_exporter();
        let span = sample_span();
        let value = exporter.encode_span(&span);

        assert_eq!(value["name"], "session.ready");
        assert_eq!(value["kind"], 1);
        assert_eq!(value["status"]["code"], 1);
        assert_eq!(value["traceId"].as_str().unwrap().len(), 32);
        assert_eq!(value["spanId"].as_str().unwrap().len(), 16);
        assert!(value.get("parentSpanId").is_none());
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
        let exporter = test_exporter();
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

    #[test]
    fn session_span_and_child_share_trace_id_with_parent_link() {
        let parent = TelemetrySpan::session_span("session.connect", SpanKind::Shell, "device-1");
        let child = TelemetrySpan::child_span(&parent, "stream.shell", SpanKind::Shell, 5);

        assert_eq!(child.trace_id, parent.trace_id);
        assert_eq!(child.parent_span_id.as_deref(), parent.span_id.as_deref());
        assert_eq!(child.stream_id, Some(5));
        assert_eq!(child.session_id, "device-1");
        assert!(parent.parent_span_id.is_none());
        assert!(parent.span_id.is_some());

        let exporter = test_exporter();
        let parent_value = exporter.encode_span(&parent);
        let child_value = exporter.encode_span(&child);
        assert!(parent_value.get("parentSpanId").is_none());
        assert_eq!(
            child_value["parentSpanId"],
            parent.span_id.as_deref().unwrap()
        );
        assert_eq!(child_value["traceId"], parent_value["traceId"]);
        assert_ne!(child_value["spanId"], parent_value["spanId"]);
    }

    #[test]
    fn collector_is_noop_without_parent() {
        let mut collector = SpanCollector::new();
        let child = collector.start_child("stream.file", SpanKind::File, 2);
        assert!(child.is_none());
        collector.finish_child(child, true, None);
        collector.finish_parent(10, true, None);
        assert!(collector.into_spans().is_empty());
    }

    #[test]
    fn collector_nests_children_under_session_parent() {
        let mut collector = SpanCollector::new();
        collector.enable_parent("session.serve", SpanKind::Session, "client-7");

        let shell = collector.start_child("stream.shell", SpanKind::Shell, 1);
        collector.finish_child(shell, true, None);
        let file = collector.start_child("stream.file", SpanKind::File, 2);
        collector.finish_child(file, false, Some("read-only root".to_owned()));

        collector.finish_parent(120, true, None);
        let spans = collector.into_spans();
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].name, "session.serve");
        assert!(spans[0].success);
        assert_eq!(spans[1].name, "stream.shell");
        assert_eq!(spans[1].stream_id, Some(1));
        assert!(spans[1].success);
        assert_eq!(spans[2].name, "stream.file");
        assert!(!spans[2].success);
        assert_eq!(spans[2].error.as_deref(), Some("read-only root"));

        let parent_id = spans[0].span_id.clone().expect("parent span id");
        assert_eq!(spans[1].parent_span_id.as_deref(), Some(parent_id.as_str()));
        assert_eq!(spans[2].parent_span_id.as_deref(), Some(parent_id.as_str()));

        let exporter = test_exporter();
        let encoded: Vec<_> = spans
            .iter()
            .map(|span| exporter.encode_span(span))
            .collect();
        assert!(encoded[0].get("parentSpanId").is_none());
        assert_eq!(encoded[1]["parentSpanId"], parent_id);
        assert_eq!(encoded[2]["status"]["code"], 2);
    }

    #[test]
    fn proxy_child_spans_encode_with_proxy_kind() {
        // Proxy dispatch is not wired into the session dispatcher yet
        // (Issue #42); the span model must still encode proxy streams.
        let mut collector = SpanCollector::new();
        collector.enable_parent("session.serve", SpanKind::Session, "client-9");
        let proxy = collector.start_child("stream.proxy", SpanKind::Proxy, 4);
        collector.finish_child(proxy, true, None);
        collector.finish_parent(30, true, None);

        let spans = collector.into_spans();
        let exporter = test_exporter();
        let value = exporter.encode_span(&spans[1]);
        let attrs = value["attributes"].as_array().unwrap();
        let kind = attrs
            .iter()
            .find(|attr| attr["key"] == "blnk.span_kind")
            .expect("span kind attribute");
        assert_eq!(kind["value"]["stringValue"], "proxy");
        let stream = attrs
            .iter()
            .find(|attr| attr["key"] == "blnk.stream_id")
            .expect("stream id attribute");
        assert_eq!(stream["value"]["intValue"], 4);
    }

    #[test]
    fn braintrust_exporter_debug_redacts_api_key() {
        let exporter = BraintrustExporter {
            endpoint: "https://otel.test/v1/traces".to_owned(),
            api_key: "secret_braintrust_key".to_owned(),
            project_id: "project_123".to_owned(),
            client: reqwest::Client::new(),
        };
        let debug_output = format!("{exporter:?}");
        assert!(!debug_output.contains("secret_braintrust_key"));
        assert!(debug_output.contains("<redacted>"));
    }
}
