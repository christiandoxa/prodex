# ADR 0976: Gateway compatibility schema must use the external migrator path

## Status

Accepted

## Context

The legacy gateway compatibility SQL helpers used request-path bootstrap to
create `prodex_gateway_*` tables. Even after removing the implicit `cfg(test)`
path, local compatibility state still depended on a request-serving bootstrap
escape hatch instead of the external migrator entrypoint.

That left the production boundary incomplete: request-serving gateway opens
could still be configured to run DDL, and `prodex-gateway migrate` did not yet
cover the same compatibility schema that gateway admin key, SCIM, usage, and
ledger adapters require.

## Decision

Move legacy gateway compatibility schema creation onto the external migrator
path and remove request-path bootstrap.

- `prodex-gateway migrate` now ensures the legacy gateway compatibility schema
  alongside the enterprise storage migration plan.
- Request-serving gateway opens always require a migrated schema and no longer
  advertise `PRODEX_GATEWAY_ALLOW_REQUEST_PATH_SCHEMA_BOOTSTRAP`.
- Gateway compatibility tests that need current SQL state prepare migrated
  schema fixtures directly before exercising load/save paths.

## Consequences

- Request-path gateway backend opens are now DDL-free in production, local
  runs, and tests.
- Fresh local SQLite/Postgres gateway state must be migrated through
  `prodex-gateway migrate` before request-serving adapters open it.
- Compatibility tests still stay cheap by creating migrated fixtures directly,
  but they now model the same open-readiness boundary as production.
- ADR 0978 follows by moving the compatibility schema itself onto explicit
  versioned migrations instead of one current-schema bootstrap SQL block.
