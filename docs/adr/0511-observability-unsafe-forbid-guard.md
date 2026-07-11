# ADR 0511: Guard observability against unsafe code

## Status

Accepted

## Context

`prodex-observability` defines telemetry contracts that cross gateway, control-plane, and storage boundaries. It must remain a side-effect-free planning crate and should not gain `unsafe` escape hatches while the enterprise refactor expands its surface.

## Decision

The observability boundary guard now requires the crate root to declare `#![forbid(unsafe_code)]`. The guard self-test covers both the accepted crate root and a negative fixture without the attribute.

## Consequences

Unsafe code cannot be introduced into the observability boundary unless the guard and this ADR are deliberately revised. This keeps telemetry contract changes reviewable and consistent with the domain, auth, gateway, application, and config boundary guards.
