# 0641: Domain Telemetry Metric Label Key Length

## Status

Accepted

## Context

Metric label values were bounded to 128 bytes, but metric label keys had no
length limit. A malformed or request-controlled key could create unbounded
exporter work even if the value was low-cardinality.

## Decision

`TelemetryAttribute::as_metric_label` now rejects metric label keys longer than
128 bytes with `MetricLabelKeyTooLong`.

## Consequences

- Metric label key names stay bounded at the domain telemetry boundary.
- Existing low-cardinality labels remain valid.
- Stable telemetry error envelopes continue to redact rejected key names and
  lengths.
