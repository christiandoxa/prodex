# ADR 0960: Application Tenant Lifecycle Debug Redaction

## Status

Accepted

## Context

Application tenant-lifecycle plans carry control-plane action details, tenant
command metadata, audit digest state, authorized/denied decisions, and nested
storage planning. Derived debug output would expose tenant and lifecycle
topology in diagnostics.

## Decision

Use custom `Debug` implementations for `ApplicationTenantLifecycleRequest`,
`ApplicationTenantLifecyclePlan`, and `ApplicationTenantLifecycleError`. Redact
control-plane action details, tenant command metadata, audit digests,
decisions, storage plans, route/resource mismatches, tenant mismatches,
operation-kind mismatches, and nested storage errors while preserving planner
and error variant names.

## Consequences

Application diagnostics can distinguish tenant-lifecycle planner shapes without
leaking tenant identifiers, audit digests, route topology, or tenant command
details.
