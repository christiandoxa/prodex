# Provider Conformance Layer v1

## Current problem

Prodex already knows a useful amount about providers inside `crates/prodex-provider-core`: provider IDs, wire formats, endpoint capability status, fallback chains, usage extraction, and catalog metadata. Gemini fallback aliases and Code Assist filtering are isolated in `fallback/chains/gemini.rs`.

But the actual request/response/stream translation behavior still lives mostly in app-side runtime modules such as:

- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_openai.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse.rs`
- related SSE readers and provider-specific runtime helper modules

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
4. add provider conformance fixtures that exercise request transforms, response transforms, stream-event transforms, usage extraction, and provider error classification
5. use the same contract later for:
   - gateway/provider routing checks
   - capability matrix validation
   - Codex protocol drift checks
   - future app-server broker compatibility work

Pure bridge helpers that decide whether provider-core output can replace a legacy app-side translation should also live in `prodex-provider-core`.
That keeps even transitional compatibility logic testable in one place instead of re-growing provider-specific predicates inside `prodex-app`.
Gemini compact summaries, content hardening, history/session import, live/realtime translation, media/content-part conversion, pre-commit stream classification, request-shape rewriting, response-shape wrapping, response/tool-call binding extraction, tool-call/tool-output shaping, tool declaration/output guard handling, value parsing, internal-instruction leak detection, and quota/error retry classification are examples of this rule: those pure guards now live under `crates/prodex-provider-core/src/gemini_bridge/`, while runtime transport policy remains in `prodex-app`. Gemini semantic compact request/response helpers live in `gemini_bridge/compact/semantic.rs`, Gemini semantic compact continuation summaries live in `gemini_bridge/compact/local/semantic.rs`, Gemini local compact tool snippet extraction lives in `gemini_bridge/compact/local/snippet/tool.rs`, Gemini assistant tool-intent guardrails live in `gemini_bridge/tooling/guardrails/intent.rs`, Gemini exact-output command matching lives in `gemini_bridge/tooling/guardrails/exact_output/commands/matching.rs`, Gemini simple-request built-in tool classification lives in `gemini_bridge/request/simple/builtin.rs`, Gemini request bridge tool/schema helpers live in `gemini_bridge/request/tools.rs`, Gemini response bridge status wrappers live in `gemini_bridge/response/status.rs`, Gemini content hardening tool-response ordering lives in `gemini_bridge/hardening/contents/tool_pairs/order.rs`, Gemini quota retry response-body parsing lives in `gemini_bridge/errors/quota_retry/body.rs`, Gemini retry-delay duration/header parsing is isolated in `gemini_bridge/errors/quota_retry/retry_delay/duration.rs`, Gemini stream pre-commit output detection lives in `gemini_bridge/precommit/output.rs`, Gemini Live G.711 codecs live in `gemini_bridge/live/audio/codec.rs`, Gemini Live audio payload conversion lives in `gemini_bridge/live/audio/payload.rs`, Gemini Live error event builders live in `gemini_bridge/live/events/error.rs`, Gemini Live output/transcript event builders live in `gemini_bridge/live/events/output.rs`, and Gemini Live setup message builders live in `gemini_bridge/live/messages/setup.rs`.
The Gemini translator request path follows the same boundary: request transform orchestration lives in `translators/gemini/request_transform.rs`, request mutation/module wiring stays in `translators/gemini/request.rs`, request tool-call thought-signature preservation lives in `translators/gemini/request/tool_signatures.rs`, request content wiring and multimodal admission checks live in `translators/gemini/request_contents.rs`, contents item shaping lives in `translators/gemini/request_contents/items.rs`, systemInstruction shaping lives in `translators/gemini/request_contents/system_instruction.rs`, contextual text detection lives in `translators/gemini/request_contents/text.rs`, continuation metadata extraction lives in `translators/gemini/request/continuation.rs`, generation-config/text-format mapping lives in `translators/gemini/request/generation_config.rs`, thinking-config mapping lives in `translators/gemini/request/generation_config/thinking.rs`, optional request field passthrough lives in `translators/gemini/request/optional_fields.rs`, response_format mapping lives in `translators/gemini/request/response_format.rs`, schema sanitization lives in `translators/gemini/request/schema.rs`, schema union/composition sanitization lives in `translators/gemini/request/schema/composition.rs`, tool declaration/tool-choice mapping lives in `translators/gemini/request/tools.rs`, and built-in request tool mapping lives in `translators/gemini/request/tools/builtin.rs`.
The Gemini translator response path is similarly split: buffered response transform orchestration lives in `translators/gemini/response_transform.rs`, Responses output assembly stays in `translators/gemini/response.rs`, chat-compatible assistant message shaping lives in `translators/gemini/response/chat_messages.rs`, finish-reason/status mapping lives in `translators/gemini/response/status.rs`, citation/web-search items live in `translators/gemini/response/grounding.rs`, usage/metadata normalization lives in `translators/gemini/response/metadata.rs`, Responses tool-call item shaping lives in `translators/gemini/response_tool_calls.rs`, chat-compatible assistant tool-call item shaping lives in `translators/gemini/response_tool_calls/chat.rs`, response tool-call shell argument wrapping lives in `translators/gemini/response_tool_calls/rtk.rs`, Gemini-specific noisy command cataloging lives in `translators/gemini/response_tool_calls/rtk/noisy.rs`, and SSE event normalization plus stream IDs, stream function-call input parsing, stream tool-call defaults/added/completed items/argument values, function-call argument deltas/signatures, chunk metadata, part parsing, response roots, output/message items, stream chat-assistant history, stream output collection, output-item added/done events, and event/value builders live in `translators/gemini/stream.rs`.
Shared OpenAI chat-compatible translation keeps bridge wiring in `translators/openai_chat_compat.rs`, supported-parameter reporting in `translators/openai_chat_compat_params.rs`, request message conversion in `translators/openai_chat_compat_request.rs`, request validation in `translators/openai_chat_compat_request/validation.rs`, Responses input content guards in `translators/openai_chat_compat_request/validation/input_content.rs`, buffered response conversion in `translators/openai_chat_compat_response.rs`, SSE event conversion in `translators/openai_chat_compat_response/stream.rs`, and shared JSON/text/tool helpers in `translators/openai_chat_compat_util.rs`.
Shared chat-compatible Responses tool translation follows the same provider-core boundary under `chat_tools_bridge/`: generic tool shapes, MCP toolset expansion, tool choice, web-search options, and shared JSON/name helpers are pure translation modules with no runtime transport state.
Shared RTK tool-argument wrapping keeps JSON argument rewriting in `translators/tool_args/rtk.rs` and shell-command insertion in `translators/tool_args/rtk/shell.rs`.
Shared provider-core bridge helpers follow the same rule: rewritten-body selection remains in `bridge.rs`, chat-compatible response/usage/request-shape mapping lives in `bridge/chat_compat.rs`, and chat-compatible tool-call mapping lives in `bridge/chat_compat/tool_calls.rs`.
DeepSeek translator helpers are split under `translators/deepseek/`: `request_transform.rs` owns request translation, `request.rs` owns request body assembly, `request/params.rs` owns request parameter validation and mapping, `response_transform.rs` owns buffered response translation, `response.rs` owns buffered response assembly, `response/metadata.rs` owns buffered response metadata extraction, `stream.rs` owns stream event translation plus stream error classification, response-created/completed event values, fallback/upstream response ID selection, response root value, stream chunk metadata extraction, stream choice metadata/delta extraction, stream metadata, output-text item IDs and item/delta values, streamed tool-call delta field extraction, streamed tool-call fallback IDs, streamed tool-call delta/function-object validation, streamed tool-call argument validation, streamed tool-call item/events, function-call argument delta source, and chat-assistant message shaping, `supported_params.rs` owns supported-parameter reporting, `tooling/messages.rs` owns request input/message orchestration, `tooling/messages/chat_items.rs` owns generic chat content and tool-call shaping, `tooling/messages/local_shell.rs` owns local-shell input call shaping, `tooling/messages/thought_signature.rs` owns tool-call thought-signature extraction, and `tooling/response_tool_calls.rs` owns response tool-call normalization.
DeepSeek bridge input/history conversion also owns basic system/user chat message shaping in `deepseek_bridge/input_items.rs`. DeepSeek bridge tool validation keeps generic tool-shape checks in `deepseek_bridge/request_tools.rs`, function-tool name/parameter checks in `deepseek_bridge/request_tools/function_tools.rs`, schema strictness in `deepseek_bridge/request_tools/strict_schema.rs`, provider-specific shape helpers in `deepseek_bridge/request_tools/shape.rs`, tool-choice validation in `deepseek_bridge/request_tools/tool_choice.rs`, and web-search option validation in `deepseek_bridge/request_tools/web_search.rs`.
Provider model catalog metadata is split by provider under `models/`, while `models.rs` keeps the lookup surface used by adapters and docs generators.
Provider contract matrix construction lives in `contract.rs`; capability Markdown rendering lives in `contract/markdown.rs`, keeping `lib.rs` as the public type/re-export surface.
DeepSeek's input/history conversion, simple-request eligibility probe, request-parameter mapping, tool-shape validation, response-message shaping, and JSON/text helpers follow the same boundary under `crates/prodex-provider-core/src/deepseek_bridge/`.

Current incremental state:

- Anthropic now has a dedicated provider-core translator with tested Responses request/response/stream chat-compat translation
- Copilot now publishes its native Responses wire contract through a dedicated passthrough translator for `responses` and `responses/compact`; request helpers, response-id extraction, and characterization tests remain split under `translators/copilot/`
- Kiro chat-completions supported-parameter reporting is split under `translators/kiro/supported_params.rs`, characterization tests live in `translators/kiro/tests.rs`, request control checks live in `translators/kiro/request/controls.rs`, chat prompt/message shaping lives in `translators/kiro/request/messages.rs`, chat content text extraction lives in `translators/kiro/request/messages/text.rs`, and request, response, and compact compatibility helpers stay in their focused Kiro modules
- Kiro conformance fixtures now cover chat-completions degraded request/response translation, invalid response-shape rejection, legacy function/tool-call mapping, accepted-control stripping for default sampling/token controls, and explicit rejection for unsupported `parallel_tool_calls=false`; Kiro provider-core characterization tests also cover chat prompt shaping, model endpoint response shaping, Kiro error body shaping, response runtime metadata decoration, response tool-call/finish-reason detection, ACP JSON-RPC request shaping, ACP model-list item shaping, ACP response-root/status shaping, ACP assistant-output/chat-assistant message shaping, ACP metadata shaping, ACP plan-entry shaping, ACP error/session-info shaping, ACP stop-reason extraction, ACP incomplete-details mapping/value shaping, Anthropic message response shaping, chat-completion stream chunk/delta/terminal-delta shaping, Responses stream event shaping, stream tool-call argument/delta bridge shaping, stream content text extraction, stream tool-call item shaping, Kiro ACP tool-call item shaping, and Kiro ACP usage-update shaping
- Gemini local rewrite shims already delegate simple-request eligibility and translated-body bridge decisions to `prodex-provider-core`
- DeepSeek local rewrite shims already delegate simple-request eligibility and retryable translated-body bridge decisions to `prodex-provider-core`
- app-side runtime modules still own auth pools, fallback loops, quota/retry policy, and transport

## What v1 owns

V1 is responsible for:

- provider capability metadata
- provider model catalog metadata
- parameter-support declarations
- pure request/response/stream translation contracts
- conformance fixtures and test execution
- usage extraction and error classification reuse
- explicit degraded/unsupported behavior instead of silent dropping

## Capability negotiation

Provider selection and request admission should follow the same order everywhere:

1. choose the provider
2. check whether the endpoint is advertised by `provider_adapter(provider).supported_endpoints()`
3. inspect endpoint status via `provider_adapter_contract_matrix()` or `capability_status(endpoint)`
4. ask `provider_translator(provider).supported_params(endpoint, model)` for parameter-level limits
5. only then run `transform_request`, `transform_response`, or `transform_stream_event`

That split is deliberate:

- adapter metadata answers "should this endpoint even be considered?"
- translator support answers "within that endpoint, what is lossless / degraded / rejected / unsupported?"
- transform results answer "what actually happened for this concrete body?"

This keeps endpoint routing, parameter admission, and body rewriting auditable as separate steps instead of hiding all three behind one opaque helper.

Model-aware gateway admission adds a catalog check between adapter/parameter support and ranking. Catalog rows may declare a context window, maximum output, default output reserve, supported reasoning efforts, and known reasoning reservation. Every added limit is optional: absent data remains `unknown`, is handled by the configured unknown-limit policy, and must never be filled with a guessed zero or a runtime scrape. Alias and combo routes are evaluated as concrete models one at a time. Direct embedding requests may use a catalog-declared embeddings endpoint, but embedding fallback remains disabled without explicit vector-space, dimensions, and normalization compatibility.

## Loss states in runtime behavior

Provider-core exposes concrete transform outcomes with `TransformStatus` and `TransformOutcome<T>`.
`ProviderTransformResult::status()` maps existing translator results to exactly:

- `lossless`
- `degraded`
- `rejected`
- `unsupported`

Every non-`lossless` status must carry a non-empty reason. The v1 conformance runner checks this through `ProviderConformanceExpectedLoss::matches_status()` and `TransformStatus::reason()`.

Provider-core loss states do not directly choose retry or routing policy. In the current runtime they map to conformance diagnostics like this:

- `lossless` → no conformance warning is logged
- `degraded but safe` → runtime logs `loss=degraded` with the translator reason
- `rejected` → runtime logs `loss=rejected` with the translator reason
- `unsupported by upstream` → runtime logs `loss=unsupported` with the translator reason

That mapping lives in the provider-bridge conformance seam. It is intentionally narrow:

- provider-core decides the semantic outcome
- runtime logging makes the downgrade/rejection observable
- transport retry / auth rotation / fallback policy still lives outside the translator

So a non-lossless transform becomes auditable immediately without giving the translator hidden control over runtime side effects.

## How to add a provider

Minimum path for a new provider in v1:

1. add the `ProviderId`
2. add model/catalog entries and fallback data
3. add a `ProviderTranslator` implementation under `src/translators/`
4. wire it into `provider_translator(provider)`
5. wire adapter metadata in `provider_adapter()` / capability status
6. add conformance fixtures for:
   - request transform
   - response transform
   - stream-event transform
   - stream completion / termination behavior when the upstream stream has an explicit terminator
   - usage extraction
   - at least one explicit non-lossless or unsupported case if the provider has limits
   - error classification, including structured error-body parsing, when the provider has distinct upstream error codes
7. update docs only after the fixtures and contract matrix agree

If runtime compatibility still needs an app-side shim, keep that shim thin and move any pure predicate or body-selection logic into `prodex-provider-core` first.

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

- deeper Anthropic and Copilot endpoint coverage beyond the current Responses path
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

The opt-in live broker now launches `codex app-server` and applies this validation bidirectionally with one shared lifecycle state before forwarding each frame. Default `prodex app-server` still passes stdio frames through unchanged, while model HTTP traffic receives the normal silent runtime-proxy preparation. The live broker does not invent provider routing or weaken continuation affinity.
