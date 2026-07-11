# 0239: Provider Pre-Commit Retry Budget

## Status

Accepted

## Context

Enterprise data-plane retries must be bounded and must not happen after a
provider request has committed output or after client cancellation. The provider
SPI already distinguishes retry stages, but adapters also need a shared retry
budget so pre-commit retry behavior does not become unbounded or adapter-specific.

## Decision

`prodex-provider-spi` owns `ProviderRetryPolicy` and `plan_provider_retry`.

The planner:

- allows retry only while the provider call is still pre-commit;
- denies retry after first provider byte or cancellation;
- denies retry when the configured pre-commit retry budget is exhausted.

## Consequences

- Provider adapters can share one retry safety contract before transport code is
  wired.
- Streaming semantics remain protected because retry after output commit is
  denied.
- Retry attempts remain bounded and explicit per adapter policy.
