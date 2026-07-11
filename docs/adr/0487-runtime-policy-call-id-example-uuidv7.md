# ADR 0487: Keep Runtime Policy Call ID Examples Globally Unique

## Status

Accepted.

## Context

Runtime gateway call IDs now use UUIDv7-backed `CallId` values. `docs/runtime-policy.md` still showed `prodex-42`, which implied the old process-local numeric request sequence was acceptable as a call identifier.

## Decision

Runtime policy documentation must describe gateway call ID headers as UUIDv7-backed values. The enterprise docs guard rejects the legacy `prodex-42` example.

## Consequences

Docs stay aligned with the globally unique ID boundary. Local numeric request sequences remain allowed for internal runtime logs and counters, but not as documented cross-replica call IDs.
