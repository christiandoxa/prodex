# ADR 0058: Redact gateway key-store load diagnostics

## Status

Accepted

## Context

The gateway data plane can load virtual-key records from file, SQLite,
PostgreSQL, or Redis state stores. Load failures and duplicate-record diagnostics
previously logged raw filesystem paths and backend errors. In regulated or
multi-tenant deployments these diagnostics can expose local state layout or
backend implementation details from security-sensitive key management paths.

## Decision

Gateway key-store load failure logs now retain event and backend information but
replace raw paths and backend errors with a stable field:

```text
error_kind=gateway_key_store_persistence_failed
```

Duplicate-record diagnostics no longer include the concrete key-store path.

## Consequences

- Operators can still identify key-store load failures and backend family.
- Runtime logs no longer expose key-store filenames, local root paths, or raw IO
  messages from the data-plane key-store load path.
- Regression coverage forces a file-backed key-store load failure and verifies
  the runtime log omits the key-store filename, temp root path, and raw directory
  IO text.
