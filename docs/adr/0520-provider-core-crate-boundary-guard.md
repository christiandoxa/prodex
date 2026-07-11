# ADR 0520: Keep provider core below orchestration

## Status

Accepted

## Context

`prodex-provider-core` is shared provider catalog and contract metadata. It should remain reusable by gateway, provider SPI, and application composition code without depending upward on `prodex-app` or runtime orchestration crates.

## Decision

Add `prodex-provider-core` to the low-level crate set enforced by `crate-boundary-guard`. The guard self-test now proves that a provider-core dependency on `prodex-app` is rejected, and npm/preflight run that self-test before scanning workspace Cargo manifests.

## Consequences

Provider metadata stays below orchestration layers. Shared provider behavior must continue to live in focused provider crates instead of importing app command handlers or terminal/runtime code.
