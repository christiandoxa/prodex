# Codex rust-v0.162.1 compatibility audit

## Disposition and scope

Prodex 0.437.2 advances the audited OpenAI Codex reference from
`rust-v0.162.0` to `rust-v0.162.1`. The release is a compatibility and release
metadata update; it does not install or replace a user Codex binary, raise the
capability-based minimum, or move Codex-owned TUI/daemon behavior into Prodex.

The upstream patch contains two fixes: multiline asynchronous questions now
preserve line breaks and hyperlink destinations, and daemon compatibility checks
compare explicit command-line feature overrides after considering supported
managed settings and feature requirements. Neither change changes the
Responses HTTP/WebSocket wire contract, quota classification, retry boundary,
hard affinity, or Prodex capacity recovery.

## Provenance

- Repository: https://github.com/openai/codex
- Stable release: https://github.com/openai/codex/releases/tag/rust-v0.162.1
- Published: 2026-10-09T19:44:51Z; not a draft or prerelease.
- Annotated tag object: `7a078f6bb4ce0dc2a81408b334ce52979d92c603`.
- Peeled commit: `092d3acd6bec3e3a14bdc7e7a2810ab628ab759d`.
- The exact source comparison has nine modified files and one added snapshot;
  no files were removed. The GitHub compare API file list is capped at 300 and
  was not used as a complete-tree inventory.

| Source archive | SHA-256 |
| --- | --- |
| `source-01620.tar.gz` | `89c5f8139486a9ce723681e977109e1512b039b7e010e2ee9a3a1a6a24952369` |
| `source-01621.tar.gz` | `e709bda93494c000a69d4c07a85a1975ce433f5541e737d1a5feddd399c97c60` |

## Reviewed upstream changes

### Multiline asynchronous questions

The TUI wraps and annotates each logical line independently, preserving CRLF
normalization, visible line breaks, and complete OSC-8 hyperlink destinations.
This is entirely Codex-owned presentation behavior. Prodex forwards Codex TUI
sessions and does not parse or rewrite asynchronous-question text.

### Daemon feature compatibility

The TUI now checks only explicit `-c features.*` overrides for a shared daemon,
while managed configuration layers and `configRequirements/read` remain
authoritative for values they own. A daemon whose defaults differ from a
client's defaults no longer causes an unnecessary restart or startup failure.
Prodex preserves Codex configuration arguments and does not implement a second
feature-compatibility policy.

### Runtime transport compatibility

The exact source comparison contains no changes to the upstream Responses,
WebSocket, retry-advice, quota, session, compaction, or app-server wire owners
covered by the Prodex boundary. Existing Prodex hard-affinity, no-midstream
rotation, and precommit capacity recovery therefore remain the applicable
runtime contracts.

## Capacity diagnosis and verification

The user-visible message `Selected model is at capacity. Please try a different
model.` is an upstream overload signal (`server_is_overloaded`), not evidence
that a profile's five-hour or weekly quota is exhausted. A quota view can show
healthy account credits while the selected model/backend is temporarily full.
Rotating profiles can help when the overload is account-local; it cannot make a
model-wide capacity shortage disappear when every profile requests the same
model.

The existing Prodex regression fixture uses the exact upstream message and
verifies that a fresh WebSocket request retries before commitment, rotates from
`main` to `second`, suppresses the retryable error from the client, and commits
the successful response. The release candidate also passed delayed/headerless
SSE capacity recovery and positive-quota local-capacity waiting regressions.
The available reproduction log records the same successful rotation and
commit. The screenshot's original runtime log was not available, so the
reproduction proves the current recovery contract but cannot establish the
screenshot's exact root cause; no production rotation/retry algorithm change
is justified by the evidence.

## Verification

- `node scripts/compat/check-upstream-baseline.mjs --self-test` — pass.
- `node scripts/compat/check-upstream-baseline.mjs` — pass.
- `node scripts/compat/check-upstream-baseline.mjs --source <exact rust-v0.162.1 extracted source directory>` — pass; 77 critical files, 86 semantic groups, and 1,238 markers replayed with zero misses.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-app --lib runtime_proxy_websocket_fresh_overload_rotates_without_leaking_retryable_error -- --test-threads=1` — pass.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-app --lib missing_content_type_capacity_rotates_before_stream_commit -- --test-threads=1` — pass.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-app --lib delayed_capacity_after_http_headers_rotates_without_restart_or_failed_binding -- --test-threads=1` — pass.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-app --lib responses_keep_waiting_after_capacity_epoch_while_quota_remains_positive -- --test-threads=1` — pass.
- `cargo check --workspace --offline` — pass.
- Official live provider/model traffic was not used.

## Subsequent quota-recovery incident

The capacity assessment above covered the earlier overload report, not the
later usage-limit incident. A subsequent real-socket regression proved a
quota-eligibility and soft-session wait-scope defect when another quota-positive
account was temporarily occupied. Prodex 0.437.2 fixes that local algorithm gap
while preserving the upstream full-context retry protocol. See
[the quota recovery and descriptor audit](quota-rotation-04372.md) for the
reproduction, limits of the original evidence, and tested behavior.
