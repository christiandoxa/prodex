# ADR 0523: Characterize combo provider fallback

## Status

Accepted

## Context

Provider fallback behavior is a compatibility invariant. `prodex-provider-core` supports explicit `combo:` fallback chains and de-duplicates repeated model names, but that behavior was not covered by a focused test.

## Decision

Add a provider-core assertion that `combo:gpt-5,gpt-5;gpt-4o` preserves order while removing duplicate entries.

## Consequences

Explicit fallback chains remain stable across providers without adding new runtime behavior.
