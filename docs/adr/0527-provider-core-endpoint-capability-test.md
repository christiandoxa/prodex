# ADR 0527: Characterize provider endpoint capabilities

## Status

Accepted

## Context

Model/provider capability negotiation is an enterprise compatibility invariant. The provider-core test verified that every provider exposes `responses` and `models`, but it did not directly cover provider-specific endpoint differences such as OpenAI/Local extended endpoints, Gemini embeddings, and DeepSeek text-only routing.

## Decision

Add focused provider-core assertions for OpenAI image support, Local A2A support, Gemini embeddings support, and DeepSeek excluding embeddings.

## Consequences

Provider endpoint capability drift is caught by the provider-core test suite without changing runtime behavior.
