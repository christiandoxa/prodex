# ADR 0276: Application Break-Glass Audit Composition

## Status

Accepted

## Context

The control-plane boundary already models break-glass access as a separate
credential scope with an explicit reason, expiry, and mandatory audit decision.
Application composition roots still needed a shared use case that persists the
authorized or denied break-glass audit event before adapters execute emergency
control-plane work.

Without an application-level composition point, adapters could call
`decide_break_glass_action` and forget to persist the resulting append-only audit
event, or could accidentally route a break-glass principal through the normal
control-plane authorization path.

## Decision

Add `plan_application_break_glass_with_audit_storage` to `prodex-application`.

The planner composes `decide_break_glass_action` with the existing
append-only audit storage plans. It selects PostgreSQL or SQLite audit storage
from the durable store configuration, persists both authorized and denied
decisions, and reuses the same stable redacted audit-storage error boundary as
normal control-plane actions.

Normal `plan_application_control_plane` remains scoped to
`CredentialScope::ControlPlane`; break-glass credentials require the explicit
break-glass planner and a non-empty, non-control-character, bounded, unexpired
authorization reason.

## Consequences

Emergency access remains separate from normal admin credentials while still
producing tenant-scoped immutable audit storage plans. Composition roots get one
reusable path for short-lived break-glass operations without introducing HTTP,
database-driver, runtime, filesystem, or provider dependencies into the
application crate.
