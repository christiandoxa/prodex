# ADR 0057: Redact gateway usage persistence diagnostics

## Status

Accepted

## Context

Gateway data-plane usage accounting persists virtual-key usage through file,
SQLite, PostgreSQL, or Redis backends. Persistence failures were logged with raw
backend errors and, for local backends, concrete filesystem paths. These logs are
security-sensitive in regulated and multi-tenant deployments because they can
expose local state layout, backend diagnostics, or operational details unrelated
to client-facing behavior.

## Decision

Gateway usage load/save failure logs now preserve the event name and backend
label but replace raw paths and backend errors with a stable field:

```text
error_kind=gateway_usage_persistence_failed
```

The data-plane behavior and backend selection are unchanged. This is a log
redaction change only.

## Consequences

- Runtime logs still show when usage persistence failed and which backend family
  was involved.
- Logs no longer expose concrete usage-state paths or raw backend error strings
  from the hot data-plane accounting path.
- Regression coverage forces a file-backed usage persistence failure and verifies
  the runtime log does not contain the state filename, root path, raw directory
  IO message, or virtual-key token.
