# ADR 0007: Gateway schema ensure is guarded per process

## Status

Accepted.

## Context

Phase 0 audit identified gateway SQLite/PostgreSQL backend open functions that
execute schema DDL every time a connection is opened. Those open functions are
used by request, admin, usage, and reconciliation paths, so repeated DDL can
appear on hot paths.

The enterprise target requires versioned migrations outside request-serving
startup. That larger migration command and compatibility check are still needed.

## Decision

As a Phase 0 compatibility-preserving hardening step, gateway SQL schema ensure
now runs at most once per backend identity in a process. The backend key is
marked only after schema ensure succeeds; failed attempts remain eligible for a
later retry.

## Consequences

- Request and reconciliation paths no longer repeat DDL after the backend schema
  has been ensured once in the running gateway process.
- Development defaults continue to bootstrap local state automatically.
- This is not the final migration design. A later phase must introduce explicit
  versioned migration files, migration status/apply commands, startup
  compatibility checks, and removal of request-serving DDL.
