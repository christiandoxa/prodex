# Codex `rust-v0.150.1` compatibility audit

This is the source record used for the `0.419.0` runtime changes. The pinned
upstream revision is `openai/codex` tag `rust-v0.150.1`; no later `main`
behavior is used as a migration assumption.

| Upstream path | Symbol/contract | Observed behavior | Prodex owner |
| --- | --- | --- | --- |
| `codex-rs/core/src/responses_retry.rs` | `ResponsesStreamRetryState`, `handle_retryable_response_stream_error` | Tracks ordinary stream retries separately from connection retries; uses `err.retry_delay()` or bounded exponential backoff; the sampling-only unbounded connection feature is gated; exhausted WebSocket retries can fall back to HTTPS. | Codex owns transport/request retry. Prodex only selects another profile at its pre-commit boundary. |
| `codex-rs/codex-api/src/sse/responses.rs` | `process_responses_event`, `process_sse_with_treatment` | `response.failed` maps explicit quota/overload errors; `response.incomplete` is surfaced as `ApiError::Stream` with its reason; `response.completed` carries optional usage; stream close before completion is an error. | Prodex does not treat every stream error as safe replay. |
| `codex-rs/core/src/client.rs` | Responses client and `ApiError`/`CodexErr` mapping | Auth, request, transport, retry-delay, and response lifecycle errors remain upstream-compatible. | Prodex preserves status/body/headers after a response and emits local `503` before an upstream response exists. |
| `codex-rs/core/src/compact_remote.rs` | `run_remote_compact_task`, `run_remote_compact_task_inner` | Remote compaction is a distinct unary lifecycle and retains session/turn metadata through its request path. | Prodex keeps compaction affinity and only retries before its response is committed. |
| `codex-rs/exec/src/exec_events.rs` | `ThreadEvent` | `codex exec --json` emits JSONL `thread.started`, `turn.started`, `item.*`, `turn.completed`, `turn.failed`, and `error` events. | `prodex ping openai` sends the user text `ping` through the normal runtime and requires a valid completed model turn; response wording is not a protocol contract. |
| `codex-rs/exec/src/event_processor_with_jsonl_output.rs` | `EventProcessorWithJsonOutput` | JSON mode writes only structured JSONL to stdout; the final agent message is taken from completed agent-message items and completion status is emitted separately. | Ping captures bounded stdout/stderr and validates these fields instead of trusting exit status. |

The upstream test evidence consulted for this audit includes the retry tests in
`responses_retry.rs`, the SSE tests around `response.failed`,
`response.incomplete`, `response.completed`, and stream-close handling in
`codex-api/src/sse/responses.rs`, and the JSON event serialization contract in
`exec/src/exec_events.rs` and `exec/src/event_processor_with_jsonl_output.rs`.

Source links:

- <https://github.com/openai/codex/blob/rust-v0.150.1/codex-rs/core/src/responses_retry.rs>
- <https://github.com/openai/codex/blob/rust-v0.150.1/codex-rs/codex-api/src/sse/responses.rs>
- <https://github.com/openai/codex/blob/rust-v0.150.1/codex-rs/core/src/client.rs>
- <https://github.com/openai/codex/blob/rust-v0.150.1/codex-rs/core/src/compact_remote.rs>
- <https://github.com/openai/codex/blob/rust-v0.150.1/codex-rs/exec/src/exec_events.rs>
- <https://github.com/openai/codex/blob/rust-v0.150.1/codex-rs/exec/src/event_processor_with_jsonl_output.rs>
- <https://github.com/openai/codex/blob/rust-v0.150.1/codex-rs/codex-api/src/rate_limits.rs>
- <https://github.com/openai/codex/blob/rust-v0.150.1/codex-rs/protocol/src/protocol.rs>
- <https://github.com/openai/codex/blob/rust-v0.150.1/codex-rs/codex-backend-openapi-models/src/models/additional_rate_limit_details.rs>

## Quota and Luna Reserve boundary

The pinned source was also checked at `codex-rs/codex-api/src/rate_limits.rs`,
`codex-rs/protocol/src/protocol.rs`, and
`codex-rs/codex-backend-openapi-models/src/models/additional_rate_limit_details.rs`.
The `RateLimitSnapshot` contract has a generic `limit_id`, `limit_name`,
windows, credits, spend-control, plan, and reached-type surface. The backend
`additional_rate_limits` entries contain `limit_name`, `metered_feature`,
`allowed`, `limit_reached`, and their windows, but the pinned source contains
no Luna Reserve identifier or model-to-bucket mapping. Prodex therefore keeps
additional buckets and their explicit admission fields, but does not infer a
Luna Reserve entitlement from plan type, model visibility, or display text.
Automatic Luna Reserve routing remains `Unknown` until a supported upstream
contract supplies that mapping; regular quota, additional buckets, credits,
and reset credits remain separate.
