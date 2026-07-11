# ADR 0502: Application readiness fails while draining

## Status

Accepted

## Context

The enterprise gateway target requires readiness to become false during graceful
shutdown draining, even when liveness, startup, policy revision, and dependency
checks are otherwise healthy.

## Decision

Application configuration readiness passes the runtime `draining` flag into the
domain health snapshot. `readyz` fails while draining, while `livez` and
`startupz` can remain passing.

The regression test
`application_configuration_readiness_snapshot_fails_readyz_while_draining`
covers this application boundary.

## Consequences

Adapters can remove a draining replica from service discovery without reporting
the process as dead or startup-incomplete.
