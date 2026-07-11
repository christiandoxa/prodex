# ADR 0241: Storage backend open readiness plan

## Status

Accepted

## Context

Enterprise gateway replicas must not run PostgreSQL or SQLite DDL while opening
request-serving backends. Earlier storage-plan boundaries already separated
versioned migration SQL from request-path DML, but adapter wiring still needs a
small, testable contract for startup/open decisions.

Without an explicit open-readiness plan, future adapters can accidentally treat a
missing schema-version check as permission to run `CREATE` or `ALTER` statements
from gateway startup or request paths.

## Decision

`prodex-storage-postgres` and `prodex-storage-sqlite` now expose backend-open
planners with three modes:

- `ExternalMigrator`: allowed to plan migrations and report DDL eligibility.
- `GatewayStartup`: requires an observed schema version at or above the required
  version and never includes migrations.
- `GatewayRequestPath`: has the same no-DDL and schema-version requirements as
  startup.

Missing schema-version checks and outdated schemas are explicit planning errors.
Request-serving open plans report zero migrations and `ddl_allowed = false`.

## Consequences

Gateway adapters can be migrated incrementally to call this readiness boundary
before opening durable request-serving backends. External migration commands keep
the only DDL-capable path.

This does not yet remove every legacy DDL-on-open call site. It narrows the next
wiring step to enforcing this planner from the connection adapter instead of
encoding migration policy directly in adapter code.
