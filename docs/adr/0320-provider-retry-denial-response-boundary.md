# ADR 0320: Provider Retry Denial Response Boundary

## Status

Accepted.

## Context

`prodex-provider-spi` already restricts retries to bounded pre-commit attempts.
When a retry is denied because output has committed, the client cancelled, or
the pre-commit retry budget is exhausted, concrete adapters still need a stable
response boundary. Raw retry plans can reveal retry stage, attempt counts,
provider routes, model names, or credential references.

## Decision

Add `ProviderRetryDecisionStatus`,
`ProviderRetryDecisionResponsePlan`, and
`plan_provider_retry_decision_response` to `prodex-provider-spi`.

Allowed retry decisions produce no response. `DeniedCommitted` maps to
`provider_retry_not_safe`; `DeniedBudgetExhausted` maps to
`provider_retry_budget_exhausted`. Both responses use stable messages and do
not include stage, attempt, provider, model, route, or credential details.

`ci:provider-spi-boundary-guard` now checks that the retry decision response
planner and stable redacted codes remain present.

## Consequences

Provider adapters can deny unsafe or exhausted retries without exposing
transport-state internals. Streaming transparency stays protected because
post-commit and cancellation retry denials remain distinguishable from allowed
pre-commit retries while still being client-safe.
