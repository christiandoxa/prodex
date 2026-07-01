# ADR 0852: Redact gateway HTTP pagination display metadata

Status: Accepted

## Context

Gateway HTTP pagination query parsing distinguishes invalid limits, duplicate
limits, and duplicate cursors. Its `Display` output named those query parameters
directly, so generic local error formatting could expose request metadata
topology outside the stable response planner.

## Decision

Keep typed variants and response planners for API compatibility, but render
pagination query parse failures with generic pagination metadata wording.

## Consequences

Client-facing response plans remain specific, while local stringified errors no
longer expose pagination query parameter names.
