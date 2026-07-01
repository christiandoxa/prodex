# ADR 0952: Storage Virtual-Key Error Debug Redaction

## Status

Accepted

## Context

Virtual-key secret reference planner errors can carry tenant and virtual-key
identifiers when storage keys do not match mutation requests. Display output is
generic, but derived debug output would expose those identifiers.

## Decision

Use a custom `Debug` implementation for `VirtualKeySecretReferencePlanError`.
Redact tenant and virtual-key identifiers while preserving error variant names.

## Consequences

Storage diagnostics can distinguish virtual-key secret reference planner
failures without leaking tenant or virtual-key identifiers from mismatch
errors.
