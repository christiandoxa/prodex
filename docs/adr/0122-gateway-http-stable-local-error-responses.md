# ADR 0122: Gateway HTTP stable local error responses

## Status

Accepted

## Context

Gateway HTTP adapters enforce body limits, method policy, trace context, and
runtime timeout policy before handing requests to provider code. These failures
are local gateway decisions, not upstream provider responses, and should not
leak parser details, exact request sizes, route enum names, or internal policy
errors to clients.

## Decision

Add `plan_gateway_http_error_response` to `prodex-gateway-http`. It maps
`GatewayHttpPlanError` values to stable status/code/message triples:

- oversized bodies become `request_body_too_large`;
- invalid methods become `method_not_allowed`;
- missing or invalid trace context becomes `invalid_trace_context`; and
- invalid local HTTP policy becomes `gateway_http_policy_invalid`.

Messages are generic and suitable for client-visible responses. Detailed error
values remain available to adapters for logs and diagnostics outside the
client-visible envelope.

## Consequences

Concrete HTTP servers can preserve upstream error compatibility while returning
secure-by-default local gateway errors for pre-upstream failures.
