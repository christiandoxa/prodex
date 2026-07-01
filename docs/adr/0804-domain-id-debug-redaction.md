# ADR 0804: Redact domain ID debug output

Status: Accepted

## Context

Typed domain IDs such as `TenantId`, `RequestId`, `CallId`, and `AuditEventId`
wrap UUIDv7 values. Their derived `Debug` formatter exposed raw identifiers to
diagnostics and containing debug output.

## Decision

Use the shared domain ID macro to emit custom `Debug` output for all typed
domain IDs. The formatter preserves the ID type name while redacting the raw
UUID value.

## Consequences

Serialization, parsing, ordering, and display remain unchanged, but raw domain
identifier UUIDs no longer appear through typed ID debug output.
