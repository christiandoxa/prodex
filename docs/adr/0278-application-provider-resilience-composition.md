# ADR 0278: Application Provider Resilience Composition

## Status

Accepted

## Context

`prodex-provider-spi` owns transport-neutral retry and circuit-breaker
primitives, but application composition roots still needed one shared use-case
boundary before concrete provider adapters perform retries or expose provider
health state.

Without an application boundary, adapters can duplicate retry stage checks,
retry after a stream has committed, or drift on circuit-breaker cooldown and
half-open behavior.

## Decision

Add application-level wrappers for provider resilience:

- `plan_application_provider_retry`
- `plan_application_provider_circuit_breaker`
- `plan_application_provider_circuit_breaker_event`

These planners delegate to the provider SPI and keep retry decisions bounded to
pre-commit stages. They also keep circuit-breaker state transitions
transport-neutral so concrete adapters can enforce cooldowns without importing
provider SDKs, async runtimes, network clients, storage drivers, or HTTP
frameworks into the application crate.

## Consequences

Composition roots have one side-effect-free resilience entry point for provider
dispatch. Streaming transparency is preserved because retry remains denied after
first byte or cancellation, while repeated provider failures can still open a
bounded circuit before new work is dispatched.
