# ADR 0086: Config boundary guard

## Status
Accepted

## Context
`prodex-config` is a boundary crate for revisioned configuration publication and
cache decisions. It must not grow concrete file loading, TOML/JSON parsing,
configuration watchers, HTTP distribution, database access, or async runtime
behavior. Those implementation concerns belong in adapter or composition crates.

## Decision
Add `scripts/ci/config-boundary-guard.mjs` and wire it into npm scripts and
local preflight. The guard requires `prodex-config` to depend only on
`prodex-domain`, forbids dev-dependencies and target-specific dependency
sections, and scans source files for forbidden filesystem, environment, process,
network, HTTP, database, transport, parser/watcher, and async-runtime imports.

## Consequences
The config boundary remains deterministic and reusable from gateway and
control-plane code. Concrete parsers, file watchers, and storage-backed
configuration distribution must be introduced in separate crates that feed
validated revisions into this boundary contract.
