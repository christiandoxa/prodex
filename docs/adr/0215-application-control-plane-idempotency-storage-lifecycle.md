# 0215: Application Control-Plane Idempotency Storage Lifecycle

## Status

Accepted

## Context

Domain replay decisions and storage idempotency plans now exist separately:
lookup, pending insert, completion update, and row materialization. Without an
application-level lifecycle plan, composition roots could execute mutating
control-plane storage before recording a pending replay marker or forget to
record completion after success.

That would weaken the enterprise requirement that retries do not double-apply
mutating admin operations.

## Decision

Add side-effect-free application plans for control-plane idempotency storage:

- `plan_application_control_plane_idempotency_storage_prepare`
- `plan_application_control_plane_idempotency_storage_complete`

The prepare plan selects the durable adapter and returns both replay lookup and
pending-record plans for the same tenant-scoped operation. The complete plan
selects the durable adapter and returns the completed-record update plan with
the serialized response bytes.

Both plans map SQL storage errors into a stable redacted application error
envelope. The application crate still performs no database I/O; `prodex-app`
or future binaries remain responsible for executing the planned statements in
the documented order.

## Consequences

- Composition roots get one explicit lifecycle for mutating admin idempotency:
  lookup durable replay state, record pending, execute mutation, then record
  completion.
- Postgres and SQLite adapter selection is centralized at the application
  boundary.
- Cross-tenant storage keys fail before mutation execution.
- Response serialization remains outside domain and storage; application plans
  carry opaque bytes.
