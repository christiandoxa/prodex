# ADR 0853: Redact gateway HTTP control-plane route display output

Status: Accepted

## Context

Control-plane route planning errors distinguish non-control-plane routes,
unknown control-plane routes, and method mismatches. Their `Display` output
named the control-plane route class, so generic error formatting could expose
route topology outside the stable response planner.

## Decision

Keep typed route errors and response planners, but render route and method
display errors with generic HTTP wording.

## Consequences

Route planning behavior and client response plans remain unchanged, while
stringified local route errors no longer expose control-plane route topology.
