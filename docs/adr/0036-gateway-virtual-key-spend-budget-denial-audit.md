# ADR 0036: Verify gateway virtual-key spend-budget denials as authorization audit events

## Status

Accepted

## Context

Gateway virtual keys may enforce a micro-USD spend budget before upstream provider
transport when an estimated model cost is known. When the estimated request cost would
exceed the key budget, the gateway returns `403 budget_exceeded`. This is a budget and
authorization boundary for tenant spend control.

Spend-budget denials are credentialed data-plane decisions. Audit records must not persist
the virtual-key bearer token.

## Decision

Spend-budget denials are verified through a regression test as `gateway_data_plane` audit
events with action `authorization_denied`, outcome `failure`, reason `budget_exceeded`,
and request path. The test uses explicit gateway route-alias model metrics so the
estimated input cost is deterministic, and the audit event omits the Authorization header
and virtual-key token value.

## Consequences

Operators can distinguish spend-budget denials from authentication, model-policy,
request-budget, RPM, and TPM denials in the immutable audit trail. Existing HTTP
compatibility remains unchanged: exhausted spend budgets still return `403 budget_exceeded`
before any upstream call.
