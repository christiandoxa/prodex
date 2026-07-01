# ADR 0531: Characterize text provider endpoint ordering

## Status

Accepted

## Context

Provider endpoint capability negotiation is an enterprise compatibility invariant. Anthropic, Copilot, and DeepSeek are intentionally text-only providers in provider-core, and their shared endpoint surface should stay deterministic.

## Decision

Add a provider-core assertion for the exact endpoint order shared by Anthropic, Copilot, and DeepSeek.

## Consequences

Text provider endpoint capability drift is caught by the provider-core test suite without changing runtime behavior.
