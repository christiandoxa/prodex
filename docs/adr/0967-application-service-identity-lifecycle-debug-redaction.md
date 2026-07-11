# ADR 0967: Application Service Identity Lifecycle Debug Redaction

## Status

Accepted

## Context

Application service-identity lifecycle plans carry control-plane action
details, service-identity command metadata, audit digest state,
authorized/denied decisions, and nested storage planning. Derived debug output
would expose tenant identifiers, service identity names, and lifecycle
topology in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationServiceIdentityLifecycleRequest`,
`ApplicationServiceIdentityLifecyclePlan`, and
`ApplicationServiceIdentityLifecycleError`. Redact control-plane action
details, service-identity command metadata, audit digests, decisions, storage
plans, route/resource mismatches, tenant mismatches, and nested storage errors
while preserving planner and error variant names.

## Consequences

Application diagnostics can distinguish service-identity lifecycle planner
shapes without leaking tenant identifiers, service identity names, or audit
digest material.
