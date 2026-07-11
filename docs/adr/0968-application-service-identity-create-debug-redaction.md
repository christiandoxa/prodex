# ADR 0968: Application Service Identity Create Debug Redaction

## Status

Accepted

## Context

Application service-identity create plans carry tenant-scoped service-identity
commands and nested storage plans. Derived debug output would expose tenant
identifiers, service identity names, and storage paths in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationServiceIdentityCreateRequest`,
`ApplicationServiceIdentityCreatePlan`, and
`ApplicationServiceIdentityCreateError`. Redact service-identity commands,
storage plans, and nested storage errors while preserving planner and error
variant names.

## Consequences

Application diagnostics can distinguish service-identity create planner shapes
without leaking tenant identifiers, service identity names, or storage paths.
