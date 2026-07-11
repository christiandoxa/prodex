# ADR 0586: Anthropic Runtime Tokens Use UUIDv7

## Status

Accepted

## Context

`prodex-runtime-anthropic` generated compatibility tokens from process ID,
current nanoseconds, and a process-local `AtomicU64` sequence. That was
reasonable for local uniqueness, but it is weaker than the enterprise
globally-unique identifier boundary and leaves collision avoidance to process
state.

The generated token prefix is compatibility-visible and should remain stable.

## Decision

Generate runtime Anthropic compatibility tokens with UUIDv7 values while
preserving the caller-provided prefix.

## Consequences

- Token values no longer depend on process ID or process-local counters.
- Existing prefix-based compatibility is preserved.
- The crate now uses the workspace `uuid` dependency already used by the domain
  typed-ID boundary.
