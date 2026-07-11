# 0229: Gateway HTTP Execution Plan

## Status

Accepted.

## Context

The enterprise gateway data plane must run on bounded async I/O with explicit
body limits, timeout budgets, cancellation propagation, streaming backpressure,
and graceful drain behavior. The framework-neutral HTTP boundary already models
route classification and policy values, but async adapters also need a single
execution contract they can apply consistently per route.

## Decision

Add `GatewayHttpExecutionPlan` and `plan_gateway_http_execution` to
`prodex-gateway-http`.

The plan carries:

- maximum request body size;
- maximum concurrent streams;
- request, stream-idle, and drain timeout budgets;
- whether cancellation propagation is required;
- whether streaming backpressure is required; and
- whether graceful drain is required.

Data-plane response and websocket routes require streaming backpressure.
Data-plane response, compact, and websocket routes require cancellation
propagation. All routes require graceful drain.

## Consequences

Concrete Axum/Hyper/Tower adapters can enforce the same bounded execution
contract without importing framework or runtime dependencies into the boundary
crate. Legacy compatibility paths remain migration targets until they execute
this plan directly.
