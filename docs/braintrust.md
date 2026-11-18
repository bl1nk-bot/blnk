# Braintrust Telemetry Integration

blnk exports per-session telemetry spans to [Braintrust](https://www.braintrust.dev)
via its hosted OpenTelemetry endpoint. The integration is opt-in, fail-open, and
does not introduce any SDK dependency — it uses a minimal OTLP/HTTP (JSON)
exporter implemented in `src/telemetry/braintrust.rs`.

## Configuration

| Env var | Required | Description |
|---|---|---|
| `BRAINTRUST_API_KEY` | Yes | Braintrust API key, sent as `Authorization: Bearer` |
| `BRAINTRUST_PROJECT_ID` | Yes | Braintrust project ID, sent as `x-bt-parent: project_id:<id>` |
| `BRAINTRUST_OTEL_ENDPOINT` | No | Override endpoint. Defaults to `https://api.braintrust.dev/otel/v1/traces`; use `https://api-eu.braintrust.dev/otel/v1/traces` for the EU data plane |

If `BRAINTRUST_API_KEY` or `BRAINTRUST_PROJECT_ID` are missing, telemetry is
silently disabled. Export failures are logged via `tracing` at `warn` and never
propagate to the CLI exit code.

## What gets exported

One span per remote session:

- `session.serve` — inbound `blnk serve` sessions (`SpanKind::Session`)
- `session.connect` — outbound shell sessions (`SpanKind::Shell`)
- `session.copy` — outbound file transfers (`SpanKind::File`)

Attributes: `blnk.session_id`, `blnk.span_kind`, `blnk.peer_id` (client or
target id), `blnk.stream_id` (when relevant), `blnk.success`, and
`error.message` on failure. Spans are batched (max 128 per request, well under
Braintrust's 10 MB OTLP payload limit) and exported with a 5-second timeout.

## Getting a key

1. Create a Braintrust account and a project.
2. Copy the project ID from the project settings page.
3. Create an API key and set both values in your environment (or the project's
   Keys tab / `.env` file).

## Security notes

- The API key is read from the environment only; it is never persisted to disk
  by blnk and never logged (payload bodies are truncated to 200 chars).
- Session spans contain no PINs, private keys, vault contents, or transferred
  file payloads — only identifiers, timings, and outcome flags, consistent with
  blnk's redaction policy in `docs/architecture.md` §10.
