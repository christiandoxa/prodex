# ADR 0030: Audit legacy gateway data-plane bearer-token failures without logging tokens

## Status

Accepted

## Context

When a gateway is configured with the legacy data-plane bearer token and no virtual keys,
requests without a valid data-plane token are rejected before they reach upstream provider
transport. This is an authentication boundary for inference traffic and is one of the
negative security cases required for enterprise deployments.

Previously this path returned `401` and wrote a runtime log marker, but it did not append
an immutable audit event. The supplied bearer token, if any, must never be persisted in an
audit record.

## Decision

Legacy gateway data-plane bearer-token failures now append a `gateway_data_plane` audit
event with action `auth_failed` and outcome `failure`. The event records the backend label,
request path, and reason `missing_or_invalid_gateway_bearer_token`.

The audit event deliberately omits the Authorization header and token material.

## Consequences

Operators can investigate failed inference authentication attempts without relying only on
runtime logs, while avoiding credential leakage. Successful data-plane requests and
upstream transport behavior remain unchanged.
