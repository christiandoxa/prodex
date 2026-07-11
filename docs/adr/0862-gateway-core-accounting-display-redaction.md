# ADR 0862: Redact gateway accounting display topology

Status: Accepted

## Context

Gateway usage reconciliation and expired reservation recovery reject tenant
mismatches before storage planning. Stable response planning already redacts
those details, but generic local display output still named tenant-mismatch
topology.

## Decision

Keep typed accounting and recovery variants for response planning and tests,
but render local display output with generic request-invalid wording.

## Consequences

Gateway accounting and recovery behavior remain unchanged, while stringified
errors no longer expose tenant topology.
