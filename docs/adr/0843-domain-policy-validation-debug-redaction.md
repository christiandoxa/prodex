# ADR 0843: Redact domain policy validation debug output

Status: Accepted

## Context

`PolicyValidation` carries digest and signature verifier outcomes. Derived
`Debug` output exposed those verifier booleans through diagnostics.

## Decision

Use a custom `Debug` implementation for `PolicyValidation` that preserves field
shape while redacting verifier results.

## Consequences

Callers still receive typed verifier outcomes, but diagnostics no longer expose
them through policy validation debug output.
