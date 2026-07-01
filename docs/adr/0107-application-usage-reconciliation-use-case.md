# ADR 0107: Application Usage Reconciliation Use Case

## Status

Accepted

## Context

The gateway core now has a post-upstream usage reconciliation boundary. The
application crate is the side-effect-free use-case orchestration layer that
future binaries and `prodex-app` composition roots should call. Without an
application-level use case, adapters could bypass the shared reconciliation
plan and reintroduce local accounting behavior.

## Decision

`prodex-application` exposes `plan_application_usage_reconciliation`. The use
case delegates to `prodex-gateway-core` and returns the planned post-provider
accounting work, including committed/released ledger events and trace-aware
gateway spans.

The application crate remains free of HTTP frameworks, async runtimes, storage
drivers, filesystem access, network clients, and provider SDKs.

## Consequences

- Composition roots have one side-effect-free entry point for post-upstream
  accounting.
- Data-plane request planning and usage reconciliation are both represented in
  the application boundary.
- Legacy adapters can migrate toward this use case without changing provider
  streaming semantics.
