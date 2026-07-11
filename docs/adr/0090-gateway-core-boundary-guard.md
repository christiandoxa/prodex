# ADR 0090: Gateway Core Boundary Guard

## Status

Accepted

## Context

`prodex-gateway-core` is the HTTP-neutral data-plane planning boundary. It
composes authentication authorization decisions, tenant-scoped atomic budget
reservation plans, provider invocation validation, and telemetry span planning.
If this crate starts depending on an HTTP framework, storage driver, async
runtime, filesystem, network client, or provider SDK, request-serving behavior
would become harder to scale horizontally and harder to verify in isolation.

The enterprise refactor is incremental, so the boundary needs an automated guard
before later gateway HTTP and storage adapter crates are introduced.

## Decision

Add `scripts/ci/gateway-core-boundary-guard.mjs` and wire its self-test plus
workspace scan into `npm run ci:preflight` through the
`gateway-core-boundary-guard` step. The guard allows only boundary dependencies
required for admission planning:

- `prodex_authz`
- `prodex_domain`
- `prodex_observability`
- `prodex_provider_core`
- `prodex_provider_spi`
- `prodex_storage`

The guard rejects direct framework/runtime/storage-driver/provider-SDK
couplings and scans gateway-core source files for filesystem, environment,
network, process, Tokio, HTTP framework, HTTP client, database, transport, and
provider SDK usage.

## Consequences

Gateway admission logic remains deterministic, side-effect free, and unit-testable.
HTTP transport, request body limits, connection pooling, database calls, Redis
rate limiting, and provider SDK integration must live in adapter crates such as
`prodex-gateway-http` or storage/provider implementations. If a future feature
needs a new dependency in `prodex-gateway-core`, it must be an explicit boundary
contract dependency and this ADR/guard must be updated in the same change.
