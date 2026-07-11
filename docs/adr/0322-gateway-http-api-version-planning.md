# ADR 0322: Gateway HTTP API Version Planning

## Status

Accepted.

## Context

Prodex control-plane and data-plane HTTP surfaces need versioned API lifecycle
governance without coupling the gateway boundary crate to Axum, Hyper, Tower, or
runtime clocks. The domain crate already owns `ApiVersionPolicy`,
`evaluate_api_version`, and stable redacted API version error envelopes, but
gateway HTTP adapters still need a framework-neutral way to derive the requested
version from path shape before routing.

## Decision

Add `GatewayHttpApiVersionPlan` and `plan_gateway_http_api_version` to
`prodex-gateway-http`.

The planner maps explicit leading path segments such as `/v2/...` to
`ApiVersion::new(2, 0)`, falls back to the configured default version for legacy
unversioned paths such as `/responses`, and delegates lifecycle decisions to the
domain `evaluate_api_version` policy. API version errors are exposed through
`plan_gateway_http_api_version_error_response`, which preserves domain redaction
and stable machine-readable codes.

## Consequences

- Concrete HTTP adapters can reject unsupported or sunset versions before
  dispatching to data-plane or control-plane handlers.
- Legacy unversioned routes remain compatible through an explicit default
  version rather than ad hoc route matching.
- Version lifecycle decisions stay in the domain crate while gateway HTTP owns
  only path-to-version extraction.
