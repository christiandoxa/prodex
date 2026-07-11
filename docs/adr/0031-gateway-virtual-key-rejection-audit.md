# ADR 0031: Audit gateway virtual-key request denials without logging tokens

## Status

Accepted

## Context

Virtual keys are the preferred data-plane authentication and policy boundary for gateway
inference traffic. Missing, unknown, or invalid virtual-key credentials return
`401 invalid_gateway_key`; model, budget, and rate-limit policy rejections return their
existing status and error code.

These denials are security-sensitive events in multi-tenant deployments. The enterprise
negative-test list explicitly calls out unknown key IDs and data-plane authorization
failures. Runtime logs are useful for diagnostics, but regulated environments need an
immutable audit trail. Request credentials must not be persisted in that trail.

## Decision

The virtual-key rejection path now appends a `gateway_data_plane` audit event before
returning the existing HTTP error. `invalid_gateway_key` is recorded as action
`auth_failed`; other virtual-key policy rejections are recorded as action
`authorization_denied`. Events include the backend label, request path, and rejection
reason, and deliberately omit the Authorization header and virtual-key token material.

## Consequences

Operators can investigate unknown-key, invalid-key, model-policy, budget, and rate-limit
denials from the audit log without leaking credentials. Existing response codes, response
bodies, transport behavior, and successful request paths remain unchanged.
