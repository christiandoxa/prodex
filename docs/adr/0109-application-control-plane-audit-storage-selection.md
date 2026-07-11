# ADR 0109: Application Control-Plane Audit Storage Selection

## Status

Accepted

## Context

Control-plane authorization decisions now produce append-only audit write
plans, and PostgreSQL/SQLite storage crates expose driver-free audit append
DML. Composition roots should not decide independently whether a successful or
denied control-plane action needs durable audit persistence.

## Decision

`prodex-application` exposes
`plan_application_control_plane_with_audit_storage`. The use case:

- evaluates the control-plane action;
- extracts the required append-only audit write from both authorized and denied
  decisions;
- selects a PostgreSQL or SQLite audit storage plan based on the durable store
  kind;
- remains side-effect-free and does not execute SQL.

## Consequences

- Security-sensitive control-plane actions have one application-level path for
  authorization and durable audit planning.
- Denied requests are audited with the same durable storage selection as
  successful mutations.
- Composition roots can migrate away from legacy ad hoc audit persistence
  without changing authorization semantics.
