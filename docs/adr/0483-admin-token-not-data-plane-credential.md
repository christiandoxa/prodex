# ADR 0483: Admin tokens are not data-plane credentials

## Status

Accepted.

## Context

Gateway admin tokens protect control-plane routes under `/v1/prodex/gateway/*`.
They must not authorize model inference or bypass virtual-key accounting.

## Decision

When virtual keys are configured, data-plane requests must authenticate with a
matching virtual key. Root/admin tokens that do not match an active virtual key
are rejected as invalid gateway keys.

## Consequences

Control-plane credentials cannot bypass data-plane accounting. Legacy bridge
token auth remains only for deployments without virtual keys.
