# ADR 0834: Redact domain deployment security report debug output

Status: Accepted

## Context

`DeploymentSecurityReport` carries the full set of deployment validation
violations. Derived `Debug` output exposed the exact violation list through
diagnostics.

## Decision

Use a custom `Debug` implementation for `DeploymentSecurityReport` that reports
only the violation count.

## Consequences

Callers still receive the typed violation list, but diagnostics no longer expose
the exact deployment security failures through report debug output.
