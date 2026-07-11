# 0655: Trace ID Exact Boundary

## Status

Accepted

## Context

`TraceId::new` and `TraceId::new_w3c` trimmed input before validation. Trace IDs
come from propagation headers and correlation adapters, so accepting padded
values can hide malformed transport behavior.

## Decision

Trace ID constructors now validate the exact provided value. Empty strings
remain empty errors, while whitespace-only or padded values are rejected as
invalid input. Accepted trace IDs are still normalized to lowercase.

## Consequences

- W3C trace context validation is exact at the domain boundary.
- Existing valid trace IDs keep working.
- Stable trace ID error responses continue to redact rejected raw values.
