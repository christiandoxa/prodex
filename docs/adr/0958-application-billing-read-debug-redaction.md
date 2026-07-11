# ADR 0958: Application Billing Read Debug Redaction

## Status

Accepted

## Context

Application billing-read plans carry control-plane action details, billing query
metadata, audit digest state, authorized/denied decisions, and nested storage
planning. Derived debug output would expose tenant and route-sensitive billing
read context in diagnostics.

## Decision

Use custom `Debug` implementations for `ApplicationBillingReadRequest`,
`ApplicationBillingReadPlan`, and `ApplicationBillingReadError`. Redact control
plane action details, billing query metadata, audit digests, decisions, storage
plans, route/resource mismatches, tenant mismatches, and nested storage errors
while preserving planner and error variant names.

## Consequences

Application diagnostics can distinguish billing-read planner shapes without
leaking tenant identifiers, audit digests, route topology, or billing query
details.
