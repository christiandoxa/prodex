# ADR 0529: Characterize Local endpoint ordering

## Status

Accepted

## Context

Provider endpoint capability negotiation is an enterprise compatibility invariant. The Local provider intentionally shares the OpenAI-compatible endpoint surface, and downstream contract rendering expects that ordering to remain deterministic.

## Decision

Add a provider-core assertion that Local endpoint ordering matches OpenAI endpoint ordering exactly.

## Consequences

Local endpoint capability drift is caught by the provider-core test suite without changing runtime behavior.
