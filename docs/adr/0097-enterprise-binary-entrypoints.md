# ADR 0097: Enterprise Binary Entrypoints

## Status

Accepted

## Context

The target architecture requires at least two operational entrypoints:
`prodex-gateway` for the data plane and `prodex-control-plane` for admin,
tenant, policy, identity, key, billing, audit, and configuration management. The
repository currently has a legacy `prodex` binary that owns CLI compatibility and
runtime launch flows. Replacing that binary or wiring a new async server in one
patch would risk behavior changes in transport transparency, streaming,
continuation affinity, and existing CLI workflows.

## Decision

Add dedicated `prodex-gateway` and `prodex-control-plane` binary entrypoints as
thin composition-root scaffolds. They expose help and version metadata, keep
`serve` explicitly gated until adapters are migrated behind
`prodex-application`, `prodex-gateway-http`, `prodex-gateway-core`, and
`prodex-control-plane`, and only wire one-shot operational commands that
delegate to existing boundary crates.

Add `scripts/ci/enterprise-binaries-guard.mjs` to ensure the two entrypoints
exist and remain thin. The guard rejects direct coupling to legacy runtime
modules, HTTP framework implementations, database drivers, and provider SDKs.
The npm and preflight paths run its explicit self-test before scanning the
workspace.

## Consequences

The workspace now has explicit binary names matching the enterprise architecture
without changing runtime traffic paths prematurely. Future adapter work can wire
these binaries to async gateway/control-plane servers with focused tests while
keeping the legacy `prodex` CLI path compatible until cutover. In the meantime,
the enterprise binaries can host small one-shot operational entrypoints such as
gateway schema migration, control-plane configuration publication planning or
delivery, and control-plane HTTP route/idempotency planning without pulling
legacy runtime code into the new boundaries. The guard self-test pins the
forbidden thin-entrypoint pattern before repository files are scanned.
