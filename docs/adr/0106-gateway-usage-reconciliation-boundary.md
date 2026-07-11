# ADR 0106: Gateway Usage Reconciliation Boundary

## Status

Accepted

## Context

The data-plane gateway reserves budget before provider invocation, but the
enterprise accounting flow also needs a post-upstream step. After completion,
cancellation, or stream interruption, the gateway must commit actual usage,
release unused reservation capacity, and propagate trace context for the
reconciliation work.

This logic must remain independent of HTTP frameworks, async runtimes, storage
drivers, and provider SDKs.

## Decision

`prodex-gateway-core` now exposes `plan_gateway_usage_reconciliation`. The plan:

- validates that the gateway tenant, storage key tenant, and reservation record
  tenant match;
- delegates accounting semantics to the storage `UsageReconciliationCommand`;
- returns the committed/released ledger plan;
- emits a `prodex.gateway.usage_reconciliation` span with propagated
  traceparent context.

## Consequences

- Gateway adapters have an explicit post-provider accounting stage to execute
  after streaming completion, cancellation, or interruption.
- Trace propagation now covers both pre-upstream reservation and post-upstream
  reconciliation.
- Concrete storage adapters still own durable SQL execution and transaction
  handling.
