# Prodex Architecture

`prodex` is not a plain pass-through proxy.

Its runtime path acts as a continuity-preserving broker that tries to stay transport-transparent to `codex` while still performing:

- profile selection
- quota-aware rotation
- continuation affinity
- overload control
- persistence and recovery
- runtime diagnostics

This document classifies the codebase by behavior-critical responsibilities rather than by current file layout.

## Core Invariants

These are the parts that define correctness.

If these fail, `prodex` can break context or diverge from upstream `codex` behavior.

### Continuation Affinity

The proxy must preserve ownership for active chains:

- `previous_response_id -> profile`
- `x-codex-turn-state -> profile`
- `session_id -> profile` for session-scoped unary routes such as remote compact

This is the main reason `prodex` cannot be reduced to a simple auth-rewriting proxy.

### Safe Rotation Boundary

Rotation is only safe before commit:

- before the first accepted unary response
- before the first committed streaming response
- before a quota-blocked or overload response is returned

No mid-stream rotation is allowed after output has started.

### Transport Transparency

The proxy should preserve upstream behavior as much as possible:

- upstream HTTP/SSE should stream directly
- websocket reuse should stay close to upstream behavior
- transport failures should prefer surfacing as transport failures, not synthetic semantic errors

### Harness Boundary

A provider identifies upstream transport, authentication, and capabilities. An account profile
identifies routing and credential state. A harness identifies model-facing canonical request policy;
it is neither a provider nor an account profile.

The resolved harness is immutable for the lifetime of a local provider bridge or gateway. It runs
after existing Prodex-owned canonical request processing and before provider wire-format
translation:

```text
canonical processing -> harness request shaping -> provider translation -> upstream transport
```

Version 1 defaults conservatively to a Native no-op. Its opt-in Minimal mode shapes only ordinary
canonical Responses inference requests. It does not add an agent loop, scan repository context,
change tools or approvals, rewrite responses or stream events, affect continuation affinity, or
move the pre-commit rotation boundary. See [docs/harness-modes.md](docs/harness-modes.md).

### State Recovery

Cross-process persistence must remain merge-safe for:

- `active_profile`
- `last_run_selected_at`
- response affinity bindings
- session affinity bindings

Continuations must survive restarts when persisted state is still valid.

## Robustness Layers

These layers are important, but they sit on top of the core invariants.

They improve behavior under load, flakiness, or quota pressure.

### Quota and Health Selection

Fresh selection currently considers:

- quota windows and pressure bands
- retry backoff
- transport backoff
- route health
- bad pairing memory
- in-flight load

This turns the proxy into a selection engine instead of a plain router.

### Admission and Overload Control

The runtime proxy also acts as a traffic governor:

- active-request limit
- long-lived queue capacity
- lane-aware admission
- overload backoff

These controls are justified when multiple terminals or mixed traffic classes compete for the same pool.

### Continuation Safeguards

Recent hardening added guards for:

- stale websocket reuse with `previous_response_id`
- compact follow-up lineage
- function-call output continuity
- negative-cache tracking for repeated `previous_response_not_found`

These are robustness features that specifically protect the core continuation invariant.

### Diagnostics

Runtime logging and `prodex doctor --runtime` are non-core for execution, but critical for operating the system safely.

They should stay accurate and cheap to maintain.

The current operational hardening extends diagnostics into a stable metrics/export layer, while keeping the runtime proxy itself transport-transparent.

## Optional Optimizers

These are the parts most open to future simplification if complexity must be reduced:

- route-specific tuning constants
- extra health and decay heuristics
- some selection tie-breakers
- Super optimizer discovery from `PRODEX_OPTIMIZERS_HOME`, `$XDG_DATA_HOME/prodex-optimizers`, and `~/.local/share/prodex-optimizers`
- some UI formatting helpers

These layers should be driven by evidence from tests and logs, not by intuition alone.

## Refactor Strategy

Refactoring should preserve behavior first and simplify second.

Recommended order:

1. Extract low-risk modules with no runtime behavior change.
2. Document invariants and failure boundaries.
3. Add regression coverage before simplifying heuristics.
4. Remove only the layers that do not improve measured reliability.

## Current Extraction Direction

The safest first extractions are support domains that are not on the hot path:

- `terminal_ui`: panel rendering and terminal layout helpers
- `runtime_config`: runtime timeout, env override, and fault-injection helpers
- `runtime_policy`: versioned local policy loading for admin-style deployments
- `runtime_doctor`: runtime diagnostic summarization and rendering
- `prodex-domain`: pure domain identifiers and security-context building blocks. It must not depend on an HTTP framework, CLI parser, database driver, filesystem, network client, or provider SDK. It owns canonical `Principal`, `TenantContext`, tenant-owned resource authorization, immutable audit-event value objects, idempotency/replay primitives, stable machine-readable error envelopes, pagination/cursor semantics, optimistic concurrency primitives, health/readiness status primitives, provider/model capability negotiation primitives, trace/correlation context primitives, migration plan/status primitives, backup/restore validation primitives, SLO/alert primitives, OIDC/JWKS validation and cache primitives, resource/action authorization matrix, explicit role mapping, credential-scope primitives, `SecretRef`, versioned policy activation/LKG value objects, and budget/accounting and rate-limit admission value objects for incremental gateway/control-plane authorization and billing hardening. `npm run ci:domain-boundary-guard` enforces this crate boundary by rejecting HTTP, CLI, database, async-runtime, transport, filesystem-adjacent, and provider/runtime dependencies in the domain manifest.

These modules reduce `main.rs` size and make it easier to reason about the runtime path without weakening behavior.

## Product Surfaces

The workspace has three composition roots with distinct ownership:

- `prodex`: local profile management and continuity-preserving Codex runtime proxy
- `prodex-gateway`: dedicated enterprise data-plane gateway
- `prodex-control-plane`: dedicated enterprise control-plane planning and publication surface

The root `src/lib.rs` is only a compatibility facade. Pure enterprise rules live in
`prodex-domain`; use cases and ports live in `prodex-application`; HTTP/server adaptation lives in
the gateway crates; backend-neutral persistence contracts live in `prodex-storage`; concrete
repositories live in the backend runtime crates. Dependencies should point inward through these
boundaries, not back into CLI or composition-root modules.

## Admin And Observability Status

What is already in place:

- secret-management abstraction for `auth.json` and profile exports, plus global secret-backend selection
- stable broker metrics export in JSON and Prometheus formats
- runtime-aware diagnostics that surface broker metrics targets without changing proxy transport semantics
- local structured audit logging for profile, rotation, and admin operations, modeled as an append-only concern alongside runtime state rather than as part of transport handling, with logs following the resolved runtime log directory by default and `PRODEX_AUDIT_LOG_DIR` as the override
- `prodex audit` as a local, read-only CLI surface for browsing recent append-only audit events

Remaining operational gaps are:

- completing production integrations around the existing RBAC, identity, governed policy, and
  configuration-publication contracts
- continued modularization of runtime-store responsibilities into smaller runtime-focused units; the `runtime_store` split is already in progress, but the boundary is still internal
- extraction of state ownership, audit/event persistence, and export helpers out of the proxy hot path
- deployment-specific non-file secret backend implementations

What remains intentionally local:

- state files and profile homes
- per-host broker ownership
- policy loading from the local filesystem
- diagnostics that are cheap enough to run on a developer workstation

## Simplification Rule

If a mechanism cannot be tied back to one of these goals, it should be considered for removal:

- preserve continuation ownership
- keep rotation pre-commit safe
- stay transport-transparent to `codex`
- fail fast under overload instead of corrupting state
- remain observable enough to debug production incidents
