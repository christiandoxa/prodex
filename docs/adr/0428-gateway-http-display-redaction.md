# ADR 0428: Redact gateway HTTP display values

## Status

Accepted.

## Context

Gateway HTTP response planning already returns stable public error envelopes,
but several local `Display` implementations still included rejected body sizes,
drain timing values, HTTP method enums, or internal route/operation names.

## Decision

Gateway HTTP `Display` text now reports the error class without rejected values
or internal enum names. Typed variants still retain structured fields for
matching and diagnostics that intentionally inspect the value.

## Consequences

Logs and adapter errors are safer by default. Callers that need exact rejected
values must use the typed error variant instead of parsing display text.
