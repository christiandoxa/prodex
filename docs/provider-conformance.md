# Provider Conformance Layer v1

## Current problem

Prodex already knows a useful amount about providers inside `crates/prodex-provider-core`: provider IDs, wire formats, endpoint capability status, fallback chains, usage extraction, and catalog metadata.

But the actual request/response/stream translation behavior still lives mostly in app-side runtime modules such as:

- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_openai.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_response.rs`
- related SSE readers and provider-specific helper modules

That split makes it hard to answer simple conformance questions from one place:

- which parameters are lossless, degraded, rejected, or unsupported for a provider/endpoint/model?
- which translated endpoints are backed by request fixtures, response fixtures, and stream fixtures?
- whether catalog/capability metadata actually matches the real transform behavior
- how to reuse the same translation contract later for gateway routing and app-server traffic

## Target architecture for v1

`prodex-provider-core` becomes the source of truth for provider conformance metadata and pure translation behavior.

The v1 shape is intentionally incremental:

1. keep runtime auth/network/state/process-launch logic in `prodex-app`
2. move pure provider translation contracts into `prodex-provider-core`
3. make transformations explicitly report one of:
   - lossless
   - degraded but safe
   - rejected
   - unsupported by upstream
4. add provider conformance fixtures that exercise request transforms, response transforms, stream-event transforms, and usage extraction
5. use the same contract later for:
   - gateway/provider routing checks
   - capability matrix validation
   - Codex protocol drift checks
   - future app-server broker compatibility work

## What v1 owns

V1 is responsible for:

- provider capability metadata
- parameter-support declarations
- pure request/response/stream translation contracts
- conformance fixtures and test execution
- usage extraction and error classification reuse
- explicit degraded/unsupported behavior instead of silent dropping

## What stays in `prodex-app`

V1 does **not** move:

- network transport
- OAuth/API-key pools
- runtime profile selection
- continuation/session binding state
- websocket lifecycle management
- quota rotation policy
- proxy admission or retry timing

Those remain app/runtime concerns. The app layer should call provider translators for pure rewrites while keeping current runtime invariants intact.

## Out of scope for v1

The following are intentionally out of scope for this phase:

- rewriting the whole gateway around a new abstraction
- changing user-facing CLI behavior
- importing LiteLLM's full model/provider catalog
- implementing the full Codex app-server broker
- replacing existing hot-path transport behavior
- adding broad dynamic provider discovery

## Follow-on work enabled by v1

Once this layer is stable, future PRs can incrementally add:

- Anthropic and Copilot translator extraction
- richer catalog-backed pricing/capability data
- contract checks against Codex Responses/app-server drift
- app-server broker request/turn normalization
- shadow-mode routing promotion based on explicit provider conformance

## App-server broker integration (design note)

The Codex app-server protocol carries structured messages between the CLI and backend: HTTP SSE responses, websocket frames, compact requests, and turn-state propagation. Prodex currently has `app_server_broker` as a skeleton that intercepts these paths selectively.

Once v1 is stable, the provider conformance layer provides the foundation for reasoning about every incoming app-server message:

### Request classification

Every incoming app-server request can be classified through a provider lens:

| App-server path | Provider endpoint | Affinity | Transform needed |
|---|---|---|---|
| `/v1/responses` (SSE) | `responses` | session, turn-state, previous_response_id | `transform_request` + `transform_stream_event` |
| `/v1/responses/compact` | `responses` (compact) | session_id | `transform_request` + `transform_response` |
| `/v1/models` | `models` | none | catalog redirect |
| WebSocket `/v1/realtime` | `responses` (streaming) | session | `transform_stream_event` |

### Turn normalization

Before the broker routes a request, it can ask the translator:

1. `supported_params(endpoint, model)` — does this provider/model accept this parameter shape?
2. `transform_request(input)` — canonicalize the request body (with explicit loss reporting)
3. `classify_error(status, code, text)` — map upstream errors to broker-level decisions

### Session affinity through the translator

The translator does not own affinity state (that remains in `prodex-app`), but the broker can use translator metadata to validate that a session-bound request still makes sense:

- `previous_response_id` → translator reports whether the provider supports turn chaining
- `x-codex-turn-state` → translator reports whether opaque state passthrough is safe
- `session_id` → broker can check endpoint capability before committing to a profile

### Protocol drift detection

The conformance fixture set acts as a canary. When Codex adds a new parameter to the Responses API, the next CI run will:

1. Exercise the new parameter against each translator's `supported_params`
2. If a translator silently drops it, the conformance case should fail or report `degraded`
3. The capability matrix regenerates, making drift visible in `docs/provider-capabilities.md`

This is intentionally out of scope for v1 implementation — the broker skeleton stays as-is — but the design direction is locked in so future changes slot cleanly into the conformance layer.
