# ADR 0827: Redact domain policy audit record debug output

Status: Accepted

## Context

`PolicyAuditRecord` carries policy revision identifiers and optional rejection
or activation reason text. Derived `Debug` output exposed both through
diagnostics.

## Decision

Use a custom `Debug` implementation for `PolicyAuditRecord` that preserves the
audit action while redacting revision identifiers and reason text.

## Consequences

Diagnostics can still distinguish validated, activated, rejected, and rollback
policy audit actions, but raw revision identifiers and free-form reason details
no longer appear through policy audit record debug output.
