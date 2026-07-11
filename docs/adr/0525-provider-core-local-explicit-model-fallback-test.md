# ADR 0525: Characterize explicit local model fallback

## Status

Accepted

## Context

Provider fallback behavior is a compatibility invariant. Local OpenAI-compatible providers share the pass-through fallback path with OpenAI, so explicit local model names should not be rewritten into provider defaults.

## Decision

Add a provider-core assertion that `Local` with `" local-model "` returns the trimmed explicit model as a single-item chain.

## Consequences

Local OpenAI-compatible model routing remains stable without changing runtime behavior.
