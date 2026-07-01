# ADR 0078: Provider SPI boundary

## Status
Accepted

## Context
The target architecture separates gateway routing and provider adapters. Existing
provider helpers describe catalogs and transformation contracts, but the gateway
also needs a stable service-provider interface that carries canonical tenant,
principal, request, call, credential-reference, route, usage-estimate, and
streaming metadata without coupling to HTTP frameworks or provider SDKs.

## Decision
Introduce `prodex-provider-spi` as a transport-neutral workspace crate. The SPI
models provider invocation envelopes, route metadata, retry-commit boundaries,
and validation rules. It depends only on `prodex-domain` and
`prodex-provider-core`.

Provider invocation validation requires:

- canonical `TenantContext` and `Principal` to match;
- data-plane credential scope for inference/provider calls;
- provider credentials to be represented as `SecretRef`; and
- retry to be allowed only before dispatch or before the first committed byte.

Control-plane and break-glass credentials are rejected at this data-plane SPI
boundary so they cannot become inference/quota bypass credentials.

## Consequences
Gateway code can migrate provider adapter calls behind this crate in small,
behavior-preserving steps. Future provider implementations get a shared contract
for tenant isolation, secret-reference usage, and no-mid-stream retry semantics
without importing database, filesystem, HTTP, async-runtime, or provider SDK
concerns into the interface layer.
