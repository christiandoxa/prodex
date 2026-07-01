# ADR 0868: Redact application control-plane tenant display output

Status: Accepted

## Context

Application audit export, billing read, and tenant lifecycle planners reject
cross-tenant requests before storage work. Stable response planning already
redacts tenant identifiers, but local display output still named tenant-mismatch
topology.

## Decision

Keep typed tenant-mismatch variants for response planning and tests, but render
local display output with the same generic request-invalid wording used by the
stable response plans.

## Consequences

Control-plane use-case behavior and response planning remain unchanged, while
stringified errors no longer expose tenant topology.
