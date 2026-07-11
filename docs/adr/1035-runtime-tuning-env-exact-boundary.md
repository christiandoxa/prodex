# ADR 1035: Runtime tuning env exact boundary

## Status

Accepted.

## Context

Runtime proxy tuning environment variables control local worker counts, queue
capacities, active-request caps, lane caps, timeouts, and pressure budgets.
These values bound hot-path concurrency and failure behavior. The shared tuning
helpers previously ignored malformed explicit env values and fell back to
policy/default values, so a production typo could silently run with a different
concurrency or timeout budget than intended.

## Decision

Shared runtime tuning env parsers now treat configured env values as exact
numeric inputs:

- empty values fail closed;
- whitespace-bearing values fail closed;
- non-numeric values fail closed;
- positive-only settings reject zero or negative values; and
- explicitly allow-zero settings keep zero as the documented escape hatch.

Policy values retain the existing compatibility behavior where non-positive
positive-only fields are ignored in favor of defaults.

## Consequences

Invalid production env wiring is visible at process startup/use instead of
silently changing local admission or timeout behavior. Operators should unset an
override to use policy/default tuning.
