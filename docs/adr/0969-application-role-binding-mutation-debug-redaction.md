# ADR 0969: Application Role-Binding Mutation Debug Redaction

## Status

Accepted

## Context

Application role-binding mutation plans carry tenant-scoped role-binding
commands and nested storage plans. Derived debug output would expose tenant
identifiers, role names, and storage paths in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationRoleBindingMutationRequest`,
`ApplicationRoleBindingMutationPlan`, and
`ApplicationRoleBindingMutationError`. Redact role-binding commands, storage
plans, and nested storage errors while preserving planner and error variant
names.

## Consequences

Application diagnostics can distinguish role-binding mutation planner shapes
without leaking tenant identifiers, role names, or storage paths.
