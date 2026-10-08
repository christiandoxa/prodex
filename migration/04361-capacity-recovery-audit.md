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

## Release gate repair: runtime-only JSON ABI

The first exact-SHA release candidate was stopped by `Real Mojo / parity` in CI
run `37744420690`. The `prodex-runtime-tuning --features mojo` consumer enables
only `prodex-mojo-core/mojo-runtime`; its thread-index protocol imported
`crate::json::JsonNode`, but that module was exposed only with `mojo-rich`.
All-features checks could not expose the missing feature boundary.

The shared JSON ABI is now available with either runtime or rich enabled,
matching the source inclusion already used by `build.rs`. No extra feature is
forced, no runtime semantics are copied to Rust, and no CI test is skipped.

The new `runtime_json_feature` integration target reproduces the compile error
before the fix and executes initialization, newest-page completion, invalid-JSON
mapping, and EOF mapping through the real Mojo protocol after the fix. CI runs
it explicitly with `--no-default-features --features mojo-runtime`.

Local PDX verification:

- Runtime-only integration: 2 passed, with the pre-fix build failing E0432.
- Original failing consumer command: `cargo test --locked -p
  prodex-runtime-tuning --features mojo -- --test-threads=1`: 8 passed.

These results supplement the capacity-recovery regressions above; they do not
claim that provider-wide capacity is guaranteed or that BRP was modified.

## Windows replay-deadline fixture qualification

Exact-SHA CI run `37752748457` completed with 65 successful jobs and one failing
Windows library partition. Its only failure was the hard-affinity rate-limit
replay fixture: expected accounts `[main, second]`, observed `[main, main, second]`.
The fixture's upstream response requested a one-second retry delay. A deliberate
2.2-second client replay delay on Linux reproduced that exact sequence, while the
initial hard-affinity request still correctly returned the full-context replay
signal after only one upstream attempt. Retrying the original profile after its
hold expires is valid production behavior, not a failure to release affinity.

The focused fixture now uses an explicit 30-second server-requested hold and
retains the 2.2-second delayed-client regression. Account-order assertions remain
strict, with an additional assertion that no alternate profile is attempted
before the client supplies full history. No production backoff, retry, or quota
policy changed. All 12 hard-affinity replay tests pass; formatting, size,
no-fallback, and Mojo authority guards pass.

## Executable-level single-profile capacity recovery

A supplemental credential-free artifact smoke on PDX starts the built
`0.436.1` executable, one synthetic Ready profile, and a loopback upstream that
flushes HTTP 200 headers, waits 200 ms, then returns `server_is_overloaded`.
The same broker retries that profile, returns a successful stream, and accepts
the next request with the same session ID without restarting. Exactly three
upstream requests occur: one failed attempt, one recovered response, and one
successful follow-up. The failed event does not reach either client response.
The unchanged general release artifact smoke also passes.

The supplemental fixture must seed a fresh Ready quota snapshot. The original
success-only fixture had no such snapshot; its synthetic credentials cannot
establish quota eligibility, so using it unchanged for recovery timed out.
Adding the synthetic snapshot corrected that fixture, without bypassing quota
checks or changing production selection policy. BRP remains read-only.
