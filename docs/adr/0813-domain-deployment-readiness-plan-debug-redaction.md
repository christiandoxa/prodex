# ADR 0813: Redact deployment readiness plan debug output

Status: Accepted

## Context

`DeploymentReadinessPlan` carries the accepted gateway replica count and
accounting-gate requirement. Its derived `Debug` formatter exposed exact
replica-count topology through diagnostics.

## Decision

Use a custom `Debug` implementation for `DeploymentReadinessPlan` that redacts
the exact replica count while preserving whether the accounting gate is required.

## Consequences

Diagnostics can still confirm that the accounting gate applies, but exact
replica-count topology no longer appears through `DeploymentReadinessPlan` debug
output.
