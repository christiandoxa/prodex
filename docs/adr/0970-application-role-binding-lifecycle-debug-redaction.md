# ADR 0970: Application Role-Binding Lifecycle Debug Redaction

## Status

Accepted

## Context

Application role-binding lifecycle plans carry control-plane action details,
role-binding mutation metadata, audit digest state, authorized/denied
decisions, and nested storage planning. Derived debug output would expose
tenant identifiers, role names, and lifecycle topology in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationRoleBindingLifecycleRequest`,
`ApplicationRoleBindingLifecyclePlan`, and
`ApplicationRoleBindingLifecycleError`. Redact control-plane action details,
role-binding mutation metadata, audit digests, decisions, storage plans,
route/resource mismatches, tenant mismatches, operation-kind mismatches, and
nested storage errors while preserving planner and error variant names.

## Consequences

Application diagnostics can distinguish role-binding lifecycle planner shapes
without leaking tenant identifiers, role names, or audit digest material.
