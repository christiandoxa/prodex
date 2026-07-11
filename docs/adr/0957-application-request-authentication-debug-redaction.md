# ADR 0957: Application Request Authentication Debug Redaction

## Status

Accepted

## Context

Application request-authentication plans carry HTTP route metadata, OIDC policy
state, JWKS cache presence, role mapping state, token claims, principals, and
scope decisions. Derived debug output would expose tenant and route topology in
diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationRequestAuthenticationRequest`,
`ApplicationRequestAuthenticationPlan`, and
`ApplicationRequestAuthenticationError`. Redact HTTP metadata, policy/cache
details, role mapping state, token claims, principals, routes, scopes, and
nested authentication errors while preserving planner and error variant names.

## Consequences

Application diagnostics can distinguish request-authentication planner shapes
without leaking tenant identifiers, credential scopes, route topology, or token
claim material.
