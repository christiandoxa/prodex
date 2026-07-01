# 0240: Provider Circuit-Breaker Plan

## Status

Accepted

## Context

Enterprise gateway adapters need circuit breakers so repeated provider failures
do not create retry storms or saturate the data plane. Circuit state should be
planned at the provider boundary without coupling to a concrete HTTP client,
runtime, or provider SDK.

## Decision

`prodex-provider-spi` owns `ProviderCircuitBreakerPolicy`,
`ProviderCircuitBreakerState`, `plan_provider_circuit_breaker`, and
`plan_provider_circuit_breaker_event`.

The planner:

- keeps the circuit closed below the failure threshold;
- opens the circuit for a bounded cooldown when failures reach the threshold;
- returns a half-open probe decision when cooldown expires;
- resets state on success.

## Consequences

- Provider adapters can share one side-effect-free circuit-breaker contract.
- Repeated upstream failures can be bounded before concrete transport retry.
- Provider route names, model names, failure counts, and cooldown internals stay
  out of client-visible behavior.
