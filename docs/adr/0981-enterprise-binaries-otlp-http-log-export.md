# ADR 0981: Enterprise binaries emit OTLP HTTP logs for live composition-root workflows

## Status

Accepted

## Context

The observability boundary already planned trace context, span descriptors, and
low-cardinality metric labels, but the dedicated enterprise binaries still had
no concrete OpenTelemetry export path of their own.

That left the gateway and control-plane composition roots with planning-only
telemetry while the actual one-shot workflows they already execute
(`migrate`, config-publication publish/deliver/consume/compact, configuration
planning, and HTTP route planning) emitted no native OTLP records.

## Decision

Wire a minimal OTLP/HTTP log exporter directly into the enterprise binaries for
their existing one-shot enterprise workflows.

- The shared helper lives in the root crate so both binaries reuse the same
  composition-root export path and the same `exported` / `disabled` / `failed`
  status mapping.
- Export is opt-in through standard OpenTelemetry environment variables:
  `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` or `OTEL_EXPORTER_OTLP_ENDPOINT`, plus
  optional `OTEL_EXPORTER_OTLP_HEADERS` and
  `OTEL_EXPORTER_OTLP_LOGS_TIMEOUT` / `OTEL_EXPORTER_OTLP_TIMEOUT`.
- `prodex-control-plane deliver-config-publication` emits one OTLP log record
  for delivery outcome.
- `prodex-control-plane plan-config-publication` and
  `plan-http-control-plane` emit one OTLP log record for planning outcome.
- `prodex-control-plane compact-config-publication` emits one OTLP log record
  for compaction outcome.
- `prodex-control-plane publish-config-publication` emits one OTLP log record
  for publication outcome.
- Successful `prodex-control-plane plan-http-control-plane` OTLP log records
  also carry the validated request, call, trace, tenant, and audit event
  correlation identifiers already produced by the shared audit-correlation
  planner.
- When a log attribute contains a valid W3C trace ID, the shared OTLP payload
  builder also sets native log-record `traceId` so collectors can correlate the
  one-shot log with traces without parsing attributes, and
  `enterprise_binaries` pins that native field on control-plane HTTP planning.
- When a log attribute contains a valid span ID, the shared OTLP payload
  builder also sets native log-record `spanId` so one-shot workflow logs can
  attach to the exact upstream span without adding a second exporter path.
  `enterprise_binaries` pins this for control-plane HTTP planning by extracting
  the span ID from the already-validated `traceparent` header.
- When a log attribute contains W3C trace flags, the shared OTLP payload
  builder sets native log-record `flags`, and `enterprise_binaries` pins this
  for control-plane HTTP planning from the validated `traceparent` header.
- Control-plane HTTP planning failure OTLP records also carry valid incoming
  `traceparent` trace ID, span ID, and flags, so route/planner failures remain
  collector-correlatable without waiting for audit correlation to succeed;
  `enterprise_binaries` pins this on route, idempotency, precondition, and page-request failures.
- `prodex-gateway migrate` emits one OTLP log record for external migration
  outcome.
- `prodex-gateway consume-config-publication` emits one OTLP log record for
  replicated consumption outcome.
- Export failure does not fail the underlying workflow; command output reports
  whether OTLP export was `disabled`, `exported`, or `failed`.
- The shared OTLP payload builder stamps every log record with
  `timeUnixNano` and `observedTimeUnixNano` so one-shot binary telemetry sorts
  correctly in downstream collectors.
- The shared OTLP payload builder also emits `severityNumber: 9` alongside
  `severityText: "INFO"` so downstream collectors receive the standard OTLP
  INFO severity without text-only inference.
- The shared OTLP payload builder mirrors the record body into low-cardinality
  `event.name` so collectors can query one-shot workflow outcomes without
  parsing log body text, and `enterprise_binaries` pins that field on
  control-plane HTTP planning.
- The shared OTLP payload builder also emits `service.version` next to
  `service.name` so one-shot gateway/control-plane telemetry can be grouped by
  deployed Prodex binary version, and `enterprise_binaries` pins that resource
  attribute on control-plane HTTP planning.
- The shared OTLP payload builder also sets instrumentation scope `version`
  from the same package version so downstream collectors can distinguish
  gateway/control-plane event schema versions, and `enterprise_binaries` pins
  that scope version on control-plane HTTP planning.
- `enterprise_binaries` pins both the explicit logs endpoint path and the
  `OTEL_EXPORTER_OTLP_ENDPOINT -> /v1/logs` fallback path for every current
  OTLP-enabled one-shot enterprise binary route, including the compatibility
  case where `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` is present but blank for every
  current route in that matrix.
- Malformed `OTEL_EXPORTER_OTLP_HEADERS` entries now fail explicitly instead of
  being ignored, while empty header fragments still stay compatibility-noisy
  but harmless. `enterprise_binaries` now pins that failure reporting on both
  the dedicated gateway and control-plane composition roots.
- Slow or hanging OTLP collectors now obey the standard OTLP timeout envs so
  one-shot enterprise commands fail OTLP export fast instead of waiting
  indefinitely on a dead collector path. `enterprise_binaries` now pins that
  timeout failure reporting on both the dedicated gateway and control-plane
  composition roots, including the generic
  `OTEL_EXPORTER_OTLP_TIMEOUT` fallback when the logs-specific timeout is unset.
- Invalid OTLP timeout env values now also fail OTLP export explicitly at those
  same composition roots instead of being silently ignored, including the
  generic `OTEL_EXPORTER_OTLP_TIMEOUT` fallback when logs-specific timeout is
  unset.

## Consequences

- The dedicated enterprise binaries now have a real OpenTelemetry export path
  without waiting for long-lived async serving.
- The exporter wiring stays small and reversible because it is attached only to
  workflows the binaries already own.
- Long-lived gateway/control-plane serving still needs the same exporter path
  carried into the future async `serve` composition root.
