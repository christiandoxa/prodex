# 0639: Domain Telemetry Printable Metric Label Values

## Status

Accepted

## Context

The domain telemetry boundary rejected high-cardinality IDs, credential-like
values, JWTs, and long metric label values. It still accepted empty labels or
labels containing whitespace, control characters, or non-ASCII bytes when a
caller bypassed typed low-cardinality enums.

## Decision

Metric label values now fail validation when they are empty or contain any
non-ASCII-graphic character. Validation is exact and does not trim before
checking secret-like prefixes.

## Consequences

- Metric exporters receive printable low-cardinality label values only.
- Raw text, multiline payloads, localized names, and accidental
  high-cardinality free-form strings fail closed at the domain boundary.
- Stable telemetry error envelopes remain redacted and do not echo rejected
  values.
