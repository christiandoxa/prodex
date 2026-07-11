# ADR 0873: Redact storage lifecycle display output

Status: Accepted

## Context

Core storage role-binding mutation, service identity create, user lifecycle,
budget policy update, and tenant lifecycle planners reject cross-tenant storage
keys before adapter work. Stable response planning already redacts tenant
identifiers, but local display output still named storage tenant-mismatch
topology.

## Decision

Keep typed storage variants for response planning and tests, but render local
display output for tenant mismatches with the same generic request-invalid
wording used by stable response plans.

## Consequences

Storage lifecycle planning behavior and response planning remain unchanged,
while stringified errors no longer expose tenant topology.
