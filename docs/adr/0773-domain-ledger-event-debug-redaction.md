# ADR 0773: Redact ledger event debug output

## Status

Accepted

## Context

`LedgerEvent` carries tenant IDs, call IDs, reservation IDs, event kind, and
usage amounts for append-only accounting writes. Derived `Debug` exposed the
tenant-scoped identifiers and usage amounts in diagnostics.

## Decision

Implement custom `Debug` for `LedgerEvent`. Preserve the ledger event kind while
redacting identifiers and usage amounts.

## Consequences

Diagnostics still show which ledger event kind was produced. Tenant IDs, call
IDs, reservation IDs, and usage amounts no longer appear through ledger-event
debug formatting.
