# ADR 0963: Application Virtual-Key Lifecycle Debug Redaction

## Status

Accepted

## Context

Application virtual-key lifecycle plans carry control-plane action details,
virtual-key secret-reference metadata, audit digest state, authorized/denied
decisions, and nested storage planning. Derived debug output would expose
tenant, virtual-key path, and rotation topology in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationVirtualKeyLifecycleRequest`,
`ApplicationVirtualKeyLifecyclePlan`, and
`ApplicationVirtualKeyLifecycleError`. Redact control-plane action details,
virtual-key reference metadata, audit digests, decisions, storage plans,
route/resource mismatches, tenant mismatches, operation-kind mismatches, and
nested secret-reference errors while preserving planner and error variant
names.

## Consequences

Application diagnostics can distinguish virtual-key lifecycle planner shapes
without leaking tenant identifiers, virtual-key paths, audit digests, or
rotation command details.
