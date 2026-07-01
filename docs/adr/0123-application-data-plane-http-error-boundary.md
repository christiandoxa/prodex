# ADR 0123: Application data-plane HTTP error boundary

## Status

Accepted

## Context

`prodex-gateway-http` now maps local HTTP policy errors to stable redacted
responses. Composition roots should not duplicate that mapping or accidentally
serialize raw application admission errors as client-visible HTTP policy
failures.

## Decision

Add `plan_application_data_plane_error_response` to `prodex-application`. It
returns the stable `GatewayHttpErrorResponsePlan` only for
`ApplicationDataPlaneError::Http` variants. Wrong-route and admission errors
remain outside this local HTTP-policy envelope so adapters can handle them with
their own authorization or routing response policy.

## Consequences

Application composition roots can reuse the secure local HTTP error mapping
without depending on an HTTP framework or leaking internal route, policy, or
admission details.
