# ADR 0863: Redact gateway authorization boundary display output

Status: Accepted

## Context

Gateway admission resolves data-plane authorization boundaries before provider
or storage planning. The typed resolver error is useful internally, but its
local display output named authorization-boundary availability directly.

## Decision

Keep the typed resolver error for fail-closed planning, but render display
output as a generic temporary gateway authorization failure.

## Consequences

Gateway authorization resolution remains unchanged, while stringified resolver
errors no longer expose boundary topology.
