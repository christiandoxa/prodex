# ADR 0522: Characterize DeepSeek provider fallback

## Status

Accepted

## Context

Provider fallback behavior is an enterprise compatibility invariant. `prodex-provider-core` already defines DeepSeek fallback chains, but the focused provider-core test covered Gemini, Anthropic, and Copilot aliases only.

## Decision

Add focused provider-core assertions for DeepSeek `flash` and `auto` fallback ordering.

## Consequences

DeepSeek fallback ordering is now protected by the provider-core test suite without changing runtime behavior.
