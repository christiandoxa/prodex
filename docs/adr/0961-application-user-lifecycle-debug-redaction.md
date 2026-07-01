# ADR 0961: Application User Lifecycle Debug Redaction

## Status

Accepted

## Context

Application user-lifecycle plans carry control-plane action details, user
command metadata, audit digest state, authorized/denied decisions, and nested
storage planning. Derived debug output would expose tenant, principal, and
directory metadata in diagnostics.

## Decision

Use custom `Debug` implementations for `ApplicationUserLifecycleRequest`,
`ApplicationUserLifecyclePlan`, and `ApplicationUserLifecycleError`. Redact
control-plane action details, user command metadata, audit digests, decisions,
storage plans, route/resource mismatches, tenant mismatches, operation-kind
mismatches, and nested storage errors while preserving planner and error
variant names.

## Consequences

Application diagnostics can distinguish user-lifecycle planner shapes without
leaking tenant identifiers, principal metadata, audit digests, or directory
command details.
