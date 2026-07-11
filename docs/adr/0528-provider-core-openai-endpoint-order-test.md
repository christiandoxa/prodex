# ADR 0528: Characterize OpenAI endpoint ordering

## Status

Accepted

## Context

Provider endpoint capability negotiation is an enterprise compatibility invariant. OpenAI-compatible routing exposes the broadest endpoint set, and downstream adapters can depend on deterministic endpoint ordering when rendering contract matrices.

## Decision

Add a provider-core assertion for the exact OpenAI endpoint order.

## Consequences

OpenAI endpoint capability ordering is now protected by the provider-core test suite without changing runtime behavior.
