# ADR 0096: Application Use-Case Boundary

## Status

Accepted

## Context

The enterprise target requires `prodex-app` to become a composition root rather
than the place where data-plane, control-plane, storage, and HTTP business logic
accumulate. Earlier slices introduced pure boundary crates for domain, auth,
gateway admission, gateway HTTP policy, control-plane planning, storage, and
provider SPI. Without an application boundary, future adapter wiring would still
need to compose those use cases inside `prodex-app`.

## Decision

Add `prodex-application` as a side-effect-free orchestration crate. It composes
existing boundary crates into application use-case plans for:

- data-plane requests: HTTP policy plus gateway admission;
- control-plane actions: control-plane authorization and audit decisions;
- runtime topology: durable storage selection, Redis-as-rebuildable-cache, and
  external-only migration mode.

The crate intentionally depends only on boundary contracts and remains free of
CLI, filesystem, network, HTTP framework, async runtime, database driver, and
provider SDK dependencies. Add `scripts/ci/application-boundary-guard.mjs` and
include it in preflight to keep that invariant explicit.

## Consequences

`prodex-app` and future `prodex-gateway`/`prodex-control-plane` binaries can
move toward thin composition roots that adapt CLI, config, HTTP, storage, and
provider clients into application plans. This keeps semantic changes separate
from mechanical adapter extraction and reduces dependency inversion pressure in
runtime modules.
