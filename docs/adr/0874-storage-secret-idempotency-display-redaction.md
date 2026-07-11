# ADR 0874: Redact storage secret and idempotency display output

Status: Accepted

## Context

Core storage virtual-key secret, provider credential, and idempotency planners
reject cross-tenant storage keys before adapter work. Stable response planning
already redacts tenant identifiers, but local display output still named storage
tenant-mismatch topology.

## Decision

Keep typed storage variants for response planning and tests, but render local
display output for tenant mismatches with the same generic request-invalid
wording used by stable response plans.

## Consequences

Secret-reference and idempotency storage planning remain unchanged, while
stringified errors no longer expose tenant topology.
