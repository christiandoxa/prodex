# 0230: Provider SPI Capability Negotiation Plan

## Status

Accepted.

## Context

The gateway must preserve model/provider capability negotiation before provider
dispatch. This decision has to stay independent from provider SDKs and transport
code so adapters cannot silently route a request to a model that lacks required
capabilities such as Responses API support, streaming, tools, websocket, or
remote compact.

`prodex-domain` already owns the redacted capability decision model. The provider
SPI needs a route-level adapter that returns a concrete `ProviderRoute` only when
the domain negotiation succeeds.

## Decision

Add provider SPI capability negotiation types:

- `ProviderRouteCapabilityCandidate`
- `ProviderCapabilityNegotiationPlan`
- `ProviderCapabilityNegotiationDecision`
- `negotiate_provider_route_capability`
- `plan_provider_capability_negotiation_error_response`

The SPI converts provider route candidates into domain model-route candidates,
delegates compatibility to `prodex-domain`, and maps a compatible decision back
to the concrete provider route.

## Consequences

Provider adapters get one side-effect-free pre-dispatch contract for capability
selection. Unsupported models or missing capabilities continue to use stable,
redacted capability errors, so provider names, model names, and missing topology
details do not leak into client-visible responses.
