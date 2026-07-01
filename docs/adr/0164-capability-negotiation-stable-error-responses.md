# 0164: Capability Negotiation Stable Error Responses

## Status

Accepted

## Context

Capability negotiation is part of the data-plane compatibility contract. The
gateway must preserve provider fallback behavior and upstream model semantics,
while still failing safely when no route supports requested capabilities such as
streaming, tools, remote compact, vision, JSON mode, or websocket transport.

The raw negotiation decision includes provider names, model names, and missing
capability sets. Those details are useful for internal routing diagnostics, but
they can leak provider topology, commercial routing choices, or compatibility
internals if returned directly to clients.

## Decision

`prodex-domain` owns `plan_capability_decision_error_response`.

The planner maps incompatible capability decisions to a stable
`model_capability_unsupported` response and maps empty route pools to
`model_route_unavailable`. Compatible decisions produce no error response. The
planner deliberately excludes provider name, model name, and missing-capability
details from the client-visible response plan.

Adapters may log the full `CapabilityDecision` through redacted diagnostics, but
must expose the stable response plan at API boundaries.

## Consequences

- Provider/model capability negotiation remains a domain-level compatibility
  primitive.
- Client-visible errors stay stable as providers or model routes change.
- Provider topology and route internals remain out of public error envelopes.
