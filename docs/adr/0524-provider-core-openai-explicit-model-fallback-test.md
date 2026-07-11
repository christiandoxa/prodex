# ADR 0524: Characterize explicit OpenAI model fallback

## Status

Accepted

## Context

Provider fallback behavior is a compatibility invariant. OpenAI-compatible explicit model names should pass through rather than being rewritten into a fallback chain.

## Decision

Add a provider-core assertion that `OpenAi` with `" custom-model "` returns the trimmed explicit model as a single-item chain.

## Consequences

Explicit OpenAI-compatible model routing remains stable without changing runtime behavior.
