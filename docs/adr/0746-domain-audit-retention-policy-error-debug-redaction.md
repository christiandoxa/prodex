# ADR 0746: Redact domain audit retention-policy error debug output

## Status

Accepted

## Context

`AuditRetentionPolicy` redacts exact retention windows in `Debug` output.
The validation error still exposed rejected retention days through derived
`Debug`. Control-plane diagnostics should preserve validation branch shape
without echoing tenant-selected retention settings.

## Decision

Implement custom `Debug` for `AuditRetentionPolicyError`. Render `TooShort.days`
and `TooLong.days` as `"<redacted>"`.

## Consequences

Diagnostics still distinguish too-short from too-long retention settings. Exact
rejected day counts remain in typed error values for programmatic handling, not
formatter output.
