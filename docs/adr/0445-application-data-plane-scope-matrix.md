# ADR 0445: Data-plane routes reject control-plane credentials

## Status

Accepted.

## Context

Control-plane credentials must not become a data-plane inference, compact,
WebSocket, or quota bypass. The application boundary is where authenticated HTTP
routes are mapped to required credential scope.

## Decision

Application authentication regression coverage now checks every current
data-plane route against a control-plane credential. Each route must fail closed
with `CredentialScopeMismatch` before request admission.

## Consequences

No runtime behavior changes. New data-plane routes should extend this route
matrix when they are added.
