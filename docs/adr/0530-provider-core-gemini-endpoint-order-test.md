# ADR 0530: Characterize Gemini endpoint ordering

## Status

Accepted

## Context

Provider endpoint capability negotiation is an enterprise compatibility invariant. Gemini supports the core text endpoints plus embeddings, and that provider-specific surface should stay deterministic for contract rendering and gateway routing.

## Decision

Add a provider-core assertion for the exact Gemini endpoint order.

## Consequences

Gemini endpoint capability drift is caught by the provider-core test suite without changing runtime behavior.
