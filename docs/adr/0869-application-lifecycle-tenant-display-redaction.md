# ADR 0869: Redact application lifecycle tenant display output

Status: Accepted

## Context

Application user lifecycle, service identity lifecycle, and budget policy
lifecycle planners reject cross-tenant commands before storage work. Stable
response planning already redacts tenant identifiers, but local display output
still named tenant-mismatch topology.

## Decision

Keep typed tenant-mismatch variants for response planning and tests, but render
local display output with the same generic request-invalid wording used by the
stable response plans.

## Consequences

Lifecycle use-case behavior and response planning remain unchanged, while
stringified errors no longer expose tenant topology.
