# ADR: Incremental `prodex-app` Decomposition

- Status: Proposed
- Date: 2026-07-15
- Owners: Prodex maintainers
- Scope: dependency and ownership changes only; no runtime behavior rewrite

## Context

`prodex-app` is the workspace composition root, but it also owns several vertical implementations.
The current source tree contains 666 Rust files and 189,288 physical Rust source lines. Its
manifest has 89 direct dependencies. Existing crate and production-boundary guards pass, so crate
count alone is not the problem. The problem is that unrelated edits share one compilation,
review, and release boundary.

The largest blast-radius contributors are:

| Contributor | Current ownership | Why it expands the blast radius |
| --- | --- | --- |
| Gateway runtime | `runtime_launch/proxy_startup`, `runtime_proxy`, gateway launch configuration | Couples HTTP, authentication, policy, storage adapters, observability, and provider dispatch. |
| Provider runtime adapters | provider-specific local rewrite, auth pool, retry, and stream modules | Couples provider SDK/crypto/HTTP dependencies to every app command. |
| Profile and provider authentication | `profile_commands`, login/import flows, provider credential refresh | Couples interactive CLI, filesystem, secret storage, and network authentication. |
| Control-plane commands | gateway/admin command handlers and wire conversion | Couples CLI rendering to gateway/application/storage contracts. |
| App-server broker tooling | broker protocol, preview validation, replay, logging, and stdio modes | Couples protocol tooling and fixtures to the runtime composition crate. |
| Prodex-owned screens | quota, dashboard, status, and doctor orchestration | High file churn and review surface even when runtime transport is unchanged. |

The current dependency direction is:

```text
root binary
  -> prodex-app                         (composition root, 89 direct dependencies)
       -> CLI/profile/UI helpers
       -> gateway HTTP/core/server + application/authn/control-plane
       -> provider core/SPI + provider runtime adapters
       -> storage ports + PostgreSQL/Redis/SQLite runtime adapters
       -> runtime launch/proxy/state/policy/observability helpers
```

Focused crates do not depend on `prodex-app`. That invariant must remain true.

## Decision

Keep `prodex-app` as the only general composition root. Extract vertical feature crates only after
their typed boundary is stable and characterized. Each extraction is a move of existing behavior,
not a redesign. `prodex-app` constructs concrete adapters, injects them into the extracted feature,
and retains top-level command routing and process launch.

No extraction may add a generic framework, a one-implementation trait, or a dependency from a
focused crate back to `prodex-app`.

## Candidate vertical crates

| Candidate | Exact ownership | Allowed dependencies | Forbidden dependencies |
| --- | --- | --- | --- |
| `prodex-app-server-broker` | JSON-RPC protocol DTOs; line parsing; typed validation failures; request/response, lifecycle, and payload validators; preview reports; contract JSON; stdio observe/validate/passthrough mechanics. Logging is accepted through a narrow event sink owned below the app. | `serde`, `serde_json`, `prodex-domain`, `prodex-redaction`; `anyhow` only at stdio entrypoints. | `prodex-app`, clap command types, gateway/runtime proxy state, provider adapters, concrete storage drivers, network clients. |
| `prodex-profile-auth` | Provider login/import/update flows, credential parsing and refresh, auth material staging, profile-name/identity integration, secret-store calls, and provider-specific auth test servers. | `prodex-core`, `prodex-profile-identity`, `prodex-shared-codex-fs`, `prodex-secret-store`, provider auth DTO crates, bounded HTTP client dependencies. | `prodex-app`, gateway runtime, runtime proxy affinity/rotation, terminal report crates, concrete governance storage adapters. |
| `prodex-provider-runtime` | Concrete provider credential pools, upstream URL/auth decoration, process/ACP lifecycle, transport assembly, provider-specific pre-commit retry state, and stream sequencing. Pure request/response/SSE transforms remain in `prodex-provider-core`; validated invocation contracts remain in `prodex-provider-spi`. | `prodex-provider-core`, `prodex-provider-spi`, provider runtime helper crates, HTTP/WebSocket/process libraries, `prodex-runtime-log`. | `prodex-app`, clap types, dashboard/quota/profile commands, gateway authorization policy, concrete governance repositories. |
| `prodex-gateway-runtime` | Live gateway composition: canonical HTTP target handoff, authentication adapter wiring, application use-case invocation, bounded admission, provider-runtime dispatch, response mapping, gateway audit/metrics wiring, and concrete storage adapter selection. | `prodex-gateway-http`, `prodex-gateway-core`, `prodex-gateway-server`, `prodex-application`, `prodex-authn`, `prodex-provider-spi`, `prodex-storage`, concrete runtime adapters at this outer boundary. | `prodex-app`, clap command definitions, profile interactive flows, quota/dashboard rendering, provider pure transform implementations. |
| `prodex-control-plane-cli` | Control-plane command handlers, command-facing request/response DTO conversion, JSON/human result models, and application port calls. Rendering primitives remain in report/UI crates. | `prodex-cli`, `prodex-application`, `prodex-control-plane`, `prodex-app-reports`, `prodex-terminal-ui`, shared DTO/domain crates. | `prodex-app`, provider transports, runtime proxy internals, database drivers, direct HTTP listeners, secret material. |

The candidate names are ownership targets, not authorization to create all five crates at once.
An extraction proceeds only when it removes a coherent implementation from `prodex-app` and its
new manifest is materially narrower than the source manifest.

## Seams that must stabilize first

The following audit refactors are prerequisites because they expose compiler-checkable or pure
boundaries without changing behavior:

1. Complete the quota TUI module ownership and remove duplicate render/runtime implementations.
2. Make `prodex-provider-core` authoritative for pure Kiro body and stream transforms.
3. Use one typed gateway admin route parser and metadata model.
4. Expose governance decision phases through request-scoped typed context.
5. Isolate the Anthropic pre-commit attempt cursor while preserving credential/model ordering.
6. Share only pre-commit loop state, not route-specific transport loops.
7. Centralize governance storage validation/codecs/integrity while keeping backend SQL explicit.
8. Modularize app-server preview around `PreviewSession` and typed `ValidationFailure`.
9. Give HTTP and WebSocket Presidio adapters one typed engine outcome.
10. Split runtime doctor marker rendering behind byte-identical golden outputs.
11. Make Super tail extraction a non-mutating typed scanner before CLI ownership moves.

An extraction PR must not also finish one of these seams. The seam refactor lands and proves parity
first; the subsequent PR moves already-stable code.

## Public API and dependency strategy

- `prodex-app` calls narrow constructors and entry functions from extracted crates. Extracted crates
  never import `prodex-app` types.
- Shared wire/domain values move only to an existing inward crate (`prodex-domain`,
  `prodex-shared-types`, `prodex-provider-spi`, `prodex-storage`, or `prodex-application`) when at
  least two real consumers need them.
- Command-only types stay in `prodex-cli`; rendering-only types stay in report/UI crates.
- Concrete adapters depend inward on ports. Ports never name concrete PostgreSQL, SQLite, Redis,
  HTTP, WebSocket, or process implementations.
- Compatibility re-exports may remain in `prodex-app` for one migration slice. They are deleted
  once all workspace callers use the new crate.
- The crate-boundary guard gains the new forbidden edges in the same PR that adds each crate.
- Feature flags are not used for permanent dual implementations. A temporary rollback selector is
  allowed only when reverting one cohesive commit cannot safely restore production wiring.

These rules prevent cycles: shared values move inward, implementations move outward, and only
`prodex-app` points across vertical features to compose them.

## Migration sequence

Every row is one PR-sized slice. Later rows do not start until the preceding row's deletion gate is
met.

| Slice | Change | Expected `prodex-app` direct-dependency reduction | Acceptance criteria | Rollback |
| --- | --- | ---: | --- | --- |
| 0. Baseline | Record `cargo tree -p prodex-app --depth 1`, file/line counts, clean and incremental timings; add dependency-edge assertions for the five candidates. | 0 | Reproducible measurements on the same toolchain/host; existing full suite and boundary guards green. | Revert measurement/guard commit. |
| 1. Broker protocol core | Create `prodex-app-server-broker`; move pure protocol, parsing, validation, report, and contract code. Keep app logging and command adapters as thin callers for the first PR, then move stdio mechanics in a second PR. | 0-2 | Contract JSON and replay fixtures byte-identical; all five mode ordering tests pass; facade has no duplicate implementation. | Repoint the facade to the prior modules and revert the extraction commit. |
| 2. Profile authentication | Extract one provider flow at a time, starting with the provider whose auth DTOs and secret-store boundary are already typed. Move common profile-auth code only after two provider slices prove the seam. | 2-5 cumulative | Login/import/update fixtures, permission checks, symlink rejection, redaction, and rollback journals unchanged. | Restore that provider's app module; persisted formats are unchanged, so no data rollback is needed. |
| 3. Provider runtime | Move one provider adapter per PR. Keep provider-core pure and gateway policy outside the crate. Move common transport helpers only after two adapters use them. | 8-15 cumulative | Provider conformance, buffered/streaming order, exact error pass-through, retry-before-commit, and no-retry-after-commit tests pass. | Revert one provider wiring commit; no protocol or persisted state shape changes. |
| 4. Gateway runtime | First move construction DTOs, then storage adapter assembly, then authenticated application dispatch. Keep `prodex-app` launch/CLI parsing as the caller. | 16-25 cumulative | Gateway security matrix, canonical-target differential tests, bounded admission, cancellation/drain, affinity, audit, and storage integration gates pass. | Repoint the app constructor to the previous in-crate composition for the affected slice. |
| 5. Control-plane CLI | Move one command family per PR: governance, identity/keys, billing/audit, then shared response conversion. | 5-10 cumulative | Exact CLI exit status and JSON/human snapshots; authorization and idempotency outcomes unchanged. | Restore that command family module and its app dispatch arm. |
| 6. Dependency cleanup | Remove compatibility re-exports, unused direct dependencies, obsolete size exceptions, and temporary boundary allowances. | Target total: at least 34 removed | `prodex-app` has no unused direct dependency and all focused crates satisfy forbidden-edge guards. | Re-add only the specific dependency/re-export required by a proven missed caller. |

The ranges are estimates because a manifest dependency can be removed only after its final app
caller moves. Each PR reports actual before/after `cargo tree` counts; no PR claims a dependency
reduction based only on moved lines.

## Test movement and characterization gates

1. Before each move, run and retain the current app-owned characterization tests.
2. Copy or move pure unit tests with their owner in the same PR. Do not weaken assertions to fit the
   new boundary.
3. Keep end-to-end CLI/runtime tests in `prodex-app` while it remains the composition root.
4. Add one facade parity test during the compatibility-re-export slice, then delete it with the old
   facade.
5. Provider/runtime moves require exact buffered response, SSE, WebSocket, error, affinity, and
   pre-commit commit-boundary coverage.
6. Storage/gateway moves require both SQLite and PostgreSQL contract tests, tenant isolation/RLS,
   atomic audit/outbox, idempotency, and persistence recovery gates.
7. Every slice runs `cargo fmt`, affected crate tests, crate/production boundary guards, the relevant
   provider/storage/gateway guard, and the full suite when the slice touches a live runtime path.

Tests move to the narrowest owning crate; integration tests remain above the boundary they prove.

## Success metrics

Current structural baseline:

- 89 direct `prodex-app` dependencies;
- 666 Rust source files;
- 189,288 physical Rust source lines.

Completion targets:

- at most 55 direct `prodex-app` dependencies, with no unused direct dependency;
- at least 80% of files in each extracted vertical owned outside `prodex-app`;
- no extracted crate with more than 35 direct dependencies without a separate ADR;
- no forbidden reverse edge or workspace dependency cycle;
- at least 30% lower median incremental `cargo check -p prodex-app` time after a leaf edit in an
  extracted vertical;
- at least 20% lower median clean `cargo check -p prodex-app` wall time;
- no regression greater than 5% in release build time or binary size unless separately approved.

Build metrics use five runs on the same host, Rust toolchain, feature set, and target directory; the
median is reported. Slice 0 records both default and production feature sets. Dependency success is
measured from Cargo metadata/tree output, not manifest line counting alone.

## Rollback policy

Each extraction must be revertible as one cohesive wiring commit after its characterization commit
has landed. Database schemas, serialized state, error codes, log markers, protocol JSON, CLI output,
and public command arguments do not change during extraction, so rollback never requires data
migration. If a temporary adapter is required, it has a deletion issue and a maximum lifetime of
two subsequent slices.

## Non-goals

- No runtime transport, retry, affinity, rotation, streaming, or error-semantics rewrite.
- No policy, routing weight, provider fallback, session, audit, or storage transaction redesign.
- No big-bang move and no arbitrary crate proliferation.
- No generic cross-provider retry framework, ORM, generic SQL builder, plugin registry, or dynamic
  handler table.
- No change to CLI flags, JSON shapes, persisted formats, log markers, audit events, or terminal
  output.
- No replacement of `prodex-app` as the composition root.

## Consequences

Compile and review blast radius becomes aligned with vertical ownership while runtime behavior and
top-level composition remain stable. The cost is temporary facade code and additional manifests,
bounded by deletion gates and dependency-count targets. A candidate that cannot remove coherent
implementation or narrow dependencies does not become a crate.
