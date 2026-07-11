# ADR 0812: Redact production readiness topology debug output

Status: Accepted

## Context

`ProductionReadinessTopology` carries replica-count and shared-store readiness
inputs. Its derived `Debug` formatter exposed raw topology details to
diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `ProductionReadinessTopology` that
redacts the exact replica count while preserving low-cardinality readiness
booleans.

## Consequences

Diagnostics can still identify whether the multi-replica accounting gate and
shared stores are configured, but exact replica-count topology no longer appears
through `ProductionReadinessTopology` debug output.
