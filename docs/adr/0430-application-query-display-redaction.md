# ADR 0430: Redact application query display values

## Status

Accepted.

## Context

Audit export and billing read errors already exposed stable response envelopes,
but their `Display` text included tenant identifiers and internal control-plane
enum names.

## Decision

Audit export and billing read display text now describes the error class
without tenant IDs or enum values. The typed variants still carry structured
fields for authorization and test assertions.

## Consequences

Application logs and adapter errors avoid leaking tenant identifiers or
internal routing classifications from cross-tenant query failures.
