# Codex rust-v0.156.0 compatibility audit

## Disposition

`CODEX_0_156_0_COMPATIBILITY=PRODEX_CLASSIFIER_UPDATE_REQUIRED`.

Codex 0.156.0 is a broad release over 0.155.1. Prodex does not need to
reimplement the new daemon, TUI, voice, workspace-routing, plugin, sandbox, or
provider-management surfaces, but the release changes two compatibility
contracts that Prodex observes directly:

- Responses HTTP and WebSocket traffic now uses the fixed `/responses` path
  without the previous `ResponsesEndpoint` selector.
- Responses failed-event classification adds three explicit balance/spend-limit
  quota codes and classifies `slow_down` as rate limiting rather than overload.

Prodex keeps the fixed Responses route transparent and updates only its
pre-commit provider error classifier so rotation/retry behavior agrees with
Codex 0.156.0.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: `rust-v0.156.0`
- Annotated tag object: `476ac1aae33835e6e4c2d311c254b9b6ce9b2f4c`
- Peeled commit: `fe74a774532af67b5a4a3dec03ce9469e17f89af`
- Release: `0.156.0`, non-prerelease, published `2026-09-22T19:51:01Z`
- Previous compatibility target: `rust-v0.155.1`
- Exact tagged-tree comparison: 3,631 changed files, 228,742 additions,
  61,996 deletions.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.155.1 source archive | `b9e18d40d322586913e94d6747f3f934922c4f5130eb5a349ba019c57b83dad8` |
| rust-v0.156.0 source archive | `a2e008b0985566188b3662f83a1c3320e791103d9e8e0a547430bc6141a4f580` |
| codex-x86_64-unknown-linux-musl.tar.gz | `3d49d9af25a5168cfc51e50e520ab238b23083c259ae7c14f89b007cb2545c7b` |
| extracted codex binary | `78a11f06e0a2dda42d13fba1d50dc62e8cbdb2d5f69789722f4d4d99b5cdbe30` |

The official Linux musl asset is 107,345,927 bytes and the extracted binary is
284,361,064 bytes.

## Exact compatibility decisions

| Surface | 0.156.0 result | Prodex decision |
| --- | --- | --- |
| Responses HTTP route | `ResponsesEndpoint` is removed from the HTTP client and requests are sent directly to `"/responses"`. | Keep `/responses` forwarding transparent. Retain `/responses/compact` only as compatibility for older clients. |
| Responses WebSocket route | WebSocket connect/probe now calls `websocket_url_for_path("/responses")` directly. | Keep the WebSocket `/responses` route, headers, turn-state affinity, and pre-commit retry boundary unchanged. |
| Root `session_id` header | Root-agent Responses requests use the prompt-cache key as the session-id header; non-root agents retain the actual session id. Actual session identity remains in turn metadata/history paths. | Treat `session_id` as an opaque upstream affinity key. Do not infer user/session identity from it or rewrite it. |
| Remote compaction v2 | Compaction still appends `CompactionTrigger` to the normal Responses input. Metadata now comes through `compaction_responses_metadata`; eligible previous-model failures may retry with the current model. | Preserve trigger/metadata opaquely, keep hard continuation affinity, and allow Codex to own model fallback semantics. |
| Explicit quota errors | `credit_balance_exhausted`, `organization_spend_limit_exceeded`, and `project_spend_limit_exceeded` join `insufficient_quota`. | Classify all four as explicit quota; rotate only before commit, pass through after commit. |
| `slow_down` | Moved from server-overload handling to rate-limit handling, including retry-after parsing. | Classify as rate limit and retry the same profile pre-commit; do not rotate it as account quota. |
| `server_is_overloaded` | Remains server overload. | Keep transient same-profile retry behavior. |
| App-server / daemon / workspace routing | The release adds and expands upstream-owned daemon, workspace routing, remote-control, provider, OAuth, plugin, and UI behavior. | Preserve additive protocol fields and capability-probed behavior. Do not duplicate these services in Prodex. |

## Verification

- Downloaded and SHA-256 verified exact `rust-v0.155.1` and
  `rust-v0.156.0` tagged source archives.
- Compared the exact tagged trees and replayed all 49 Prodex critical-file
  assertions against the 0.156.0 source; all 49 pass after updating the
  intentional fixed-route and compaction-metadata assertions.
- Verified the GitHub release metadata and annotated/peeled tag identities.
- Downloaded the official Linux musl asset; its size and SHA-256 match the
  release metadata.
- The extracted official binary reports `codex-cli 0.156.0`.
- `codex app-server --help` exposes daemon, proxy, TypeScript generation, and
  JSON-schema generation.
- An isolated stdio app-server `initialize` with
  `capabilities.experimentalApi=true` returned a valid 0.156.0 result from a
  private synthetic home without credentials or a model turn.
- Prodex runtime-proxy error-policy tests pass with the new quota/rate codes:
  25 passed under the active Mojo path.
- Provider-core classifier tests pass: 6 passed.
- The app native-first-event regression passes for `slow_down` and the new
  explicit quota family.
- The offline upstream baseline self-test and normal guard pass with
  `rust-v0.156.0` as the tested release.

The audit did not replace or modify the user's installed Codex binary,
credentials, or live orchestrator.

## Prodex 0.431 preparation

This audit supersedes the 0.155.1 compatibility pin for the Prodex 0.431
release train. Prodex keeps upstream-owned Codex behavior external and limits
the compatibility repair to classification and pinned contract metadata.
