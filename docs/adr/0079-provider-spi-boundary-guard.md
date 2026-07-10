# ADR 0079: Provider SPI boundary guard

## Status
Accepted

## Context
`prodex-provider-spi` is intended to remain a transport-neutral boundary between
the gateway data plane and provider adapters. Without an automated guard, future
changes could accidentally introduce HTTP frameworks, database drivers, provider
SDKs, filesystem access, network access, or async runtimes into the SPI layer.
That would violate the target architecture and make the interface difficult to
reuse from gateway, control-plane, and storage composition roots.

## Decision
Add `scripts/ci/provider-spi-boundary-guard.mjs` and wire its self-test plus
workspace scan into npm scripts and local preflight. The guard enforces that
`prodex-provider-spi` depends only on
`prodex-domain` and `prodex-provider-core`, forbids target-specific dependency
sections, and scans source files for forbidden boundary imports such as
filesystem, network, HTTP, database, transport, async-runtime, and provider SDK
usage.

## Consequences
The provider SPI can evolve incrementally while preserving dependency inversion.
If an implementation needs HTTP, database, or SDK behavior, it must live in a
lower adapter/composition crate rather than the SPI contract itself.
