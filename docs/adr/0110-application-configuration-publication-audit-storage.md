# ADR 0110: Application Configuration Publication Audit Storage

## Status

Accepted

## Context

Configuration publication is a control-plane operation that changes the policy
or configuration revision seen by gateway replicas. The control-plane boundary
already validates publication and emits audit write plans, and storage crates
provide append-only audit SQL plans. Composition roots should not bypass this
shared path for accepted or rejected configuration publication attempts.

## Decision

`prodex-application` exposes `plan_application_configuration_publication`. The
use case:

- evaluates configuration publication through `prodex-control-plane`;
- uses the audit write from authorized and denied publication decisions;
- selects PostgreSQL or SQLite append-only audit storage plans;
- remains side-effect-free and does not execute SQL or access files/network.

## Consequences

- Configuration revision publishing has an application-level audit persistence
  plan.
- Rejected publication attempts are durably audited like successful
  publications.
- Composition roots can wire storage execution without duplicating
  authorization, publication validation, or audit selection logic.
