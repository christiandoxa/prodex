# ADR 0971: Application Virtual-Key Secret Reference Debug Redaction

## Status

Accepted

## Context

Application virtual-key secret-reference plans carry tenant-scoped virtual-key
commands and nested storage plans. Derived debug output would expose tenant
identifiers, virtual-key names, and storage paths in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationVirtualKeySecretReferenceRequest`,
`ApplicationVirtualKeySecretReferencePlan`, and
`ApplicationVirtualKeySecretReferenceError`. Redact virtual-key commands,
storage plans, and nested storage errors while preserving planner and error
variant names.

## Consequences

Application diagnostics can distinguish virtual-key secret-reference planner
shapes without leaking tenant identifiers, virtual-key names, or storage
paths.
