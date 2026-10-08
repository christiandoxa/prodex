# 0.436.1 capacity recovery audit

## Evidence and limits

Read-only incident analysis on 2026-10-08 observed repeated local precommit-budget
503 responses, a Codex WebSocket-to-HTTP fallback, and five subsequent HTTP 200
responses that surfaced the client capacity error. A fresh/resumed client later
made progress. No credentials, account identifiers, session IDs, request bodies,
or private deployment information are included here.

The official Codex `rust-v0.161.0` sources explain the client state transition:
`codex-rs/core/src/client.rs::force_http_fallback` sets the client's
`disable_websockets` atomic for its lifetime. In
`codex-rs/codex-api/src/sse/responses_error.rs`, `server_is_overloaded` maps to
`ApiError::ServerOverloaded`. The affected proxy's complete upstream event trace
was not available; the exact initial cause of those earlier 503s is not claimed.

## Independently reproduced defects

1. The HTTP SSE lookahead converted Hold to Commit on its short deadline or byte
   threshold. A mock upstream that returned headers and then a delayed overload
   reproduced the capacity frame escaping rather than triggering profile retry.
2. After preserving precommit state, a single-profile recovery test exposed a
   second defect: the retained unsuccessful streaming reply kept its admission
   lease. The local profile remained at its hard limit while the retry loop
   waited for capacity held by that same reply. This is a resource-lifetime bug,
   not evidence of insufficient RAM or exhausted account quota.

## Production changes

- Mojo `prodex_runtime_sse_precommit_boundary_v1` owns the finite wait/abort
  decision for held prefixes. Rust only polls I/O, records elapsed time, and
  applies the selected result. A poll interval never authorizes commit.
- The existing event classifier remains the sole commit/error classification
  owner. A commit-ready event is forwarded without waiting for the full stream.
- Explicit quota, rate-limit, and overload SSE failures drop their host admission
  lease before entering the outer retry loop. Their response payload is retained
  for existing forwarding behavior without retaining profile capacity.
- Previous-response recovery keeps its intentional guard transfer, and a
  successful stream retains its lease until completion/drop. No Rust semantic
  fallback, account bypass, model downgrade, or raised concurrency limit exists.

## Reproduction and regression commands

```sh
PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib delayed_capacity -- --test-threads=1
PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib failed_sse_attempt_releases_slot_while_payload_is_retained -- --test-threads=1
PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib runtime_proxy::prefetch -- --test-threads=1
PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib responses_overload_recovery -- --test-threads=1
PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib runtime_proxy::response_forwarding -- --test-threads=1
PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-mojo-core --features mojo-core sse_precommit --lib
node scripts/ci/runtime-test-manifest-guard.mjs
node scripts/ci/mojo-production-share.mjs
```

The regression for delayed headers failed before the fix with the actual mock
`response.failed` payload. After the fixes, rotation and the single-profile
same-session follow-up both succeeded. Remaining release qualification is
tracked by exact-SHA CI and the standalone release workflow, not this document.

## Release freshness qualification

The first standalone release dispatch stopped before artifact creation because
Caveman 3.2.0 and Ponytail 5.0.0 had replaced the previous stable references.
The release gate was retained. Both official tagged source archives were
validated against the existing bounded-tree contract (regular files only,
no symlinks, unchanged file/count/byte limits), and the exact tag/commit/Git-tree
and Prodex tree digests were recorded in `optional-tools-audit.json`.

The compiled production resolvers accepted each exact tree, rejected a modified
tree, and accepted the restored original. No hooks, telemetry, installers, or
upstream binaries were executed. All 59 optional-tool tests passed; old minimum
version and legacy-manifest rules were unchanged. These are reference updates,
not automatic installation or activation of an optional tool.
