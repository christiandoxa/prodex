# ADR 0521: Route provider core changes through enterprise ID guard

## Status

Accepted

## Context

`prodex-provider-core` is now part of the enterprise ID boundary scan. Focused changed-test selection still used its own crate list, so provider-core source changes could miss the process-local identifier guard unless a broader CI path ran.

## Decision

Add `prodex-provider-core` to the changed-tests enterprise ID boundary crate set and extend the existing impact test to cover `crates/prodex-provider-core/src/lib.rs`.

## Consequences

Provider-core changes now trigger the same enterprise ID guard as domain, gateway, auth, storage, and provider SPI boundary changes.
