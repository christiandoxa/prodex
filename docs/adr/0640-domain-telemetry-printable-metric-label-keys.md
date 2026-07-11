# 0640: Domain Telemetry Printable Metric Label Keys

## Status

Accepted

## Context

Metric label values were constrained to printable low-cardinality strings, and
identifier-like label keys were rejected. The key guard still accepted empty
keys or keys containing whitespace, control characters, or non-ASCII bytes
before applying the high-cardinality denylist.

## Decision

Metric label keys now fail validation when they are empty or contain any
non-ASCII-graphic character. Validation is exact and does not trim before
checking the high-cardinality denylist.

## Consequences

- Exporters receive printable metric label keys only.
- Malformed keys cannot bypass the high-cardinality denylist or create backend
  specific label parsing behavior.
- Stable telemetry error envelopes remain redacted and do not echo rejected
  keys.
