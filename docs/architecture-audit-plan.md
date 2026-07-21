# Prodex Architecture Audit and Milestone Plan

> Historical refactor journal. Current architecture and supported behavior are
> documented in `docs/architecture.md`, `docs/provider-conformance.md`, and
> `docs/enterprise-governance/implementation-ledger.md`.

This note captures the current repository state before the next structural refactor.

It is based on the current worktree, not intent.

Related current docs:

- `docs/provider-conformance.md`
- `docs/provider-capabilities.md`

## Short audit

### 1. Where provider transforms currently live

`crates/prodex-provider-core` already owns the canonical pure translation contract:

- `src/translator.rs`
- `src/translator/transform.rs`
- `src/translator/transform/status.rs`
- `src/translators/openai_chat_compat.rs`
- `src/translators/openai_chat_compat_params.rs`
- `src/translators/openai_chat_compat_request.rs`
- `src/translators/openai_chat_compat_request/validation.rs`
- `src/translators/openai_chat_compat_request/validation/input_content.rs`
- `src/translators/openai_chat_compat_response.rs`
- `src/translators/openai_chat_compat_response/stream.rs`
- `src/translators/openai_chat_compat_util.rs`
- `src/translators/anthropic.rs`
- `src/translators/copilot.rs`
- `src/translators/copilot/request.rs`
- `src/translators/copilot/response.rs`
- `src/translators/copilot/tests.rs`
- `src/translators/gemini.rs`
- `src/translators/gemini/request.rs`
- `src/translators/gemini/request/tool_signatures.rs`
- `src/translators/gemini/request_contents.rs`
- `src/translators/gemini/request_contents/items.rs`
- `src/translators/gemini/request_contents/system_instruction.rs`
- `src/translators/gemini/request_contents/text.rs`
- `src/translators/gemini/request/continuation.rs`
- `src/translators/gemini/request/generation_config.rs`
- `src/translators/gemini/request/generation_config/thinking.rs`
- `src/translators/gemini/request/optional_fields.rs`
- `src/translators/gemini/request/response_format.rs`
- `src/translators/gemini/request/schema.rs`
- `src/translators/gemini/request/schema/composition.rs`
- `src/translators/gemini/request/tools.rs`
- `src/translators/gemini/request/tools/builtin.rs`
- `src/translators/gemini/response.rs`
- `src/translators/gemini/response/build.rs`
- `src/translators/gemini/response/grounding.rs`
- `src/translators/gemini/response/metadata.rs`
- `src/translators/gemini/response/status.rs`
- `src/translators/gemini/response_tool_calls.rs`
- `src/translators/gemini/response_tool_calls/chat.rs`
- `src/translators/gemini/response_tool_calls/rtk.rs`
- `src/translators/gemini/response_tool_calls/rtk/noisy.rs`
- `src/translators/deepseek.rs`
- `src/translators/deepseek/request_transform.rs`
- `src/translators/deepseek/response_transform.rs`
- `src/translators/deepseek/stream.rs`
- `src/translators/deepseek/supported_params.rs`
- `src/translators/deepseek/tooling.rs`
- `src/translators/deepseek/tooling/messages.rs`
- `src/translators/deepseek/tooling/response_tool_calls.rs`
- `src/translators/kiro.rs`
- `src/translators/kiro/supported_params.rs`
- `src/translators/kiro/tests.rs`
- `src/translators/kiro/request.rs`
- `src/translators/kiro/response.rs`
- `src/translators/kiro/compact.rs`
- `src/translators/passthrough.rs`
- `src/contract.rs`
- `src/bridge.rs`
- `src/bridge/chat_compat.rs`
- `src/bridge/chat_compat/response.rs`
- `src/bridge/chat_compat/tool_calls.rs`
- `src/bridge/tests.rs`
- `src/bridge/chat_compat/usage.rs`
- `src/errors.rs`
- `src/errors/body.rs`
- `src/usage.rs`
- `src/usage/estimate.rs`
- `src/models.rs`
- `src/models/anthropic.rs`
- `src/models/copilot.rs`
- `src/models/copilot/catalog.rs`
- `src/models/deepseek.rs`
- `src/models/gemini.rs`
- `src/models/gemini/catalog.rs`
- `src/models/kiro.rs`
- `src/models/local.rs`
- `src/models/openai.rs`
- `src/fallback.rs`
- `src/fallback/body.rs`
- `src/fallback/chains.rs`
- `src/fallback/chains/gemini.rs`
- `src/chat_tools_bridge.rs`
- `src/chat_tools_bridge/tool_choice.rs`
- `src/chat_tools_bridge/tools.rs`
- `src/chat_tools_bridge/tools/custom.rs`
- `src/chat_tools_bridge/tools/namespace.rs`
- `src/chat_tools_bridge/tools/tool_search.rs`
- `src/chat_tools_bridge/util.rs`
- `src/chat_tools_bridge/web_search.rs`
- `src/gemini_bridge.rs`
- `src/gemini_bridge/bindings.rs`
- `src/gemini_bridge/compact.rs`
- `src/gemini_bridge/compact/semantic.rs`
- `src/gemini_bridge/compact/local.rs`
- `src/gemini_bridge/compact/local/semantic.rs`
- `src/gemini_bridge/compact/local/snippet.rs`
- `src/gemini_bridge/compact/local/snippet/tool.rs`
- `src/gemini_bridge/compact/local/text.rs`
- `src/gemini_bridge/errors.rs`
- `src/gemini_bridge/errors/quota_retry.rs`
- `src/gemini_bridge/errors/quota_retry/body.rs`
- `src/gemini_bridge/errors/quota_retry/retry_delay.rs`
- `src/gemini_bridge/errors/quota_retry/retry_delay/duration.rs`
- `src/gemini_bridge/hardening.rs`
- `src/gemini_bridge/hardening/contents.rs`
- `src/gemini_bridge/hardening/contents/parts.rs`
- `src/gemini_bridge/hardening/contents/tool_pairs.rs`
- `src/gemini_bridge/hardening/contents/tool_pairs/order.rs`
- `src/gemini_bridge/hardening/thought_signature.rs`
- `src/gemini_bridge/history.rs`
- `src/gemini_bridge/history/session_import.rs`
- `src/gemini_bridge/leaks.rs`
- `src/gemini_bridge/leaks/echo.rs`
- `src/gemini_bridge/leaks/patterns.rs`
- `src/gemini_bridge/leaks/patterns/catalog.rs`
- `src/gemini_bridge/live.rs`
- `src/gemini_bridge/live/audio.rs`
- `src/gemini_bridge/live/audio/codec.rs`
- `src/gemini_bridge/live/audio/payload.rs`
- `src/gemini_bridge/live/events.rs`
- `src/gemini_bridge/live/events/error.rs`
- `src/gemini_bridge/live/events/output.rs`
- `src/gemini_bridge/live/messages.rs`
- `src/gemini_bridge/live/messages/setup.rs`
- `src/gemini_bridge/media.rs`
- `src/gemini_bridge/media/content_object.rs`
- `src/gemini_bridge/media/mime.rs`
- `src/gemini_bridge/precommit.rs`
- `src/gemini_bridge/precommit/output.rs`
- `src/gemini_bridge/request.rs`
- `src/gemini_bridge/request/exact_output.rs`
- `src/gemini_bridge/request/native_project.rs`
- `src/gemini_bridge/request/simple.rs`
- `src/gemini_bridge/request/simple/builtin.rs`
- `src/gemini_bridge/request/tools.rs`
- `src/gemini_bridge/response.rs`
- `src/gemini_bridge/response/simple.rs`
- `src/gemini_bridge/response/status.rs`
- `src/gemini_bridge/response/tool_calls.rs`
- `src/gemini_bridge/tests.rs`
- `src/gemini_bridge/tool_io.rs`
- `src/gemini_bridge/tool_io/command_response.rs`
- `src/gemini_bridge/tool_io/mask.rs`
- `src/gemini_bridge/tooling.rs`
- `src/gemini_bridge/tooling/gemini3.rs`
- `src/gemini_bridge/tooling/guardrails.rs`
- `src/gemini_bridge/tooling/guardrails/exact_output/commands.rs`
- `src/gemini_bridge/tooling/guardrails/exact_output/commands/matching.rs`
- `src/gemini_bridge/tooling/guardrails/exact_output.rs`
- `src/gemini_bridge/tooling/guardrails/intent.rs`
- `src/gemini_bridge/tooling/guardrails/tool_text.rs`
- `src/gemini_bridge/util.rs`
- `src/deepseek_bridge.rs`
- `src/deepseek_bridge/input_items.rs`
- `src/deepseek_bridge/input_items/history.rs`
- `src/deepseek_bridge/input_items/push.rs`
- `src/deepseek_bridge/input_items/push/fields.rs`
- `src/deepseek_bridge/input_items/validation.rs`
- `src/deepseek_bridge/input_items/validation/content.rs`
- `src/deepseek_bridge/json.rs`
- `src/deepseek_bridge/messages.rs`
- `src/deepseek_bridge/messages/adjacency.rs`
- `src/deepseek_bridge/request_params.rs`
- `src/deepseek_bridge/request_params/metadata.rs`
- `src/deepseek_bridge/request_params/reasoning.rs`
- `src/deepseek_bridge/request_params/reject.rs`
- `src/deepseek_bridge/request_probe.rs`
- `src/deepseek_bridge/request_probe/input.rs`
- `src/deepseek_bridge/request_tools.rs`
- `src/deepseek_bridge/request_tools/function_tools.rs`
- `src/deepseek_bridge/request_tools/shape.rs`
- `src/deepseek_bridge/request_tools/shape/mcp.rs`
- `src/deepseek_bridge/request_tools/strict_schema.rs`
- `src/deepseek_bridge/request_tools/tool_choice.rs`
- `src/deepseek_bridge/request_tools/tool_shape.rs`
- `src/deepseek_bridge/request_tools/web_search.rs`
- `src/deepseek_bridge/tests.rs`
- `tests/provider_conformance_v1.rs`
- `tests/fixtures/provider_conformance_cases.json`

Current contract status:

- explicit transform outcomes already exist as:
  - `Lossless`
  - `DegradedButSafe`
  - `Rejected`
  - `UnsupportedUpstream`
- request / response / stream translation exists
- usage extraction exists
- error classification exists

But provider-specific runtime logic still leaks through `prodex-app`, especially under:

- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_deepseek.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse/completion.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse/tool_calls.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_state.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_state/complete.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_state/events.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_state/output.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/chat_compatible_request.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_response_chat_compatible.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro.rs`
- `crates/prodex-app/src/runtime_launch/proxy_startup/provider_tools.rs`

`provider_bridge.rs` and `provider_bridge_conformance.rs` are already the intended seam, but they still bridge into large app-side rewrite code instead of making provider-core the only owner of pure translation.

Current incremental bridge status:

- Gemini local rewrite already consults provider-core for request conformance and simple-request/body replacement via:
  - `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini.rs`
  - `crates/prodex-provider-core/src/gemini_bridge.rs`
- Gemini SSE state now delegates stream IDs, stream function-call input parsing, stream tool-call defaults/added/completed item/argument-value shaping, function-call argument delta/signature shaping, chunk metadata/part extraction, response-root/output-message/history/output collection shaping, output-item added/done event shaping, and event/value shapes for created/completed/incomplete/metadata/text/reasoning events to provider-core while app code keeps only state sequencing and transport assembly.
- DeepSeek local rewrite already consults provider-core for request conformance and simple-request/body replacement via:
  - `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_deepseek.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge.rs`
- DeepSeek input/history conversion, basic chat message shaping, simple-request probing, request-parameter mapping, reasoning-mode mapping, tool-shape validation, strict tool-schema normalization, response-message shaping, and JSON/text compatibility helpers are now split into provider-core submodules:
  - `crates/prodex-provider-core/src/deepseek_bridge/input_items.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/input_items/history.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/input_items/push.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/input_items/push/fields.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/input_items/validation.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/input_items/validation/content.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_probe.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_probe/input.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_params.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_params/metadata.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_params/reasoning.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_params/reject.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_tools.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_tools/function_tools.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_tools/shape.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_tools/shape/mcp.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_tools/strict_schema.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_choice.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_shape.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/request_tools/web_search.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/messages.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/messages/adjacency.rs`
  - `crates/prodex-provider-core/src/deepseek_bridge/json.rs`
- `crates/prodex-provider-core/src/deepseek_bridge.rs` is now a small re-export module; its characterization tests live in `crates/prodex-provider-core/src/deepseek_bridge/tests.rs`.
- Gemini compact summary handling, local compact fallback/continuation summaries, content hardening, history/session import, live/realtime translation, live audio config/codec conversion, live OpenAI-compatible event builders, media/content-part conversion, pre-commit stream classification, request-shape rewriting, response-shape wrapping, response/tool-call binding extraction, tool-call/tool-output shaping, Gemini-3 tool declaration overrides, tool-output guard handling, exact command-output forcing, value parsing, leak filtering, normalized error shaping, and quota/rate-limit retry classification are now split into provider-core submodules:
  - `crates/prodex-provider-core/src/gemini_bridge/bindings.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/compact.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/compact/semantic.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/compact/local.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/compact/local/semantic.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/compact/local/snippet.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/compact/local/snippet/tool.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/compact/local/text.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/errors.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/errors/quota_retry.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/errors/quota_retry/body.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/errors/quota_retry/retry_delay.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/errors/quota_retry/retry_delay/duration.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/hardening.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/hardening/contents.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/hardening/contents/parts.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/hardening/contents/tool_pairs.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/hardening/contents/tool_pairs/order.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/hardening/thought_signature.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/history.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/history/session_import.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/leaks.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/leaks/echo.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/leaks/patterns.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/leaks/patterns/catalog.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/live.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/live/audio.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/live/audio/codec.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/live/audio/payload.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/live/events.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/live/events/error.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/live/events/output.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/live/messages.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/live/messages/setup.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/media.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/media/content_object.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/media/mime.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/precommit.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/precommit/output.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/request.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/request/exact_output.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/request/native_project.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/request/simple.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/request/simple/builtin.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/request/tools.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/response.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/response/simple.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/response/status.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/response/tool_calls.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tool_io.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tool_io/command_response.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tool_io/mask.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tooling.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tooling/gemini3.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails/exact_output/commands.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails/exact_output/commands/matching.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails/exact_output.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails/intent.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails/tool_text.rs`
  - `crates/prodex-provider-core/src/gemini_bridge/util.rs`
- `crates/prodex-provider-core/src/gemini_bridge.rs` is now a small re-export module; its characterization tests live in `crates/prodex-provider-core/src/gemini_bridge/tests.rs`.
- Gemini provider translator request helpers are also split by pure request responsibility:
  - `crates/prodex-provider-core/src/translators/gemini/request_transform.rs` owns request transform orchestration
  - `crates/prodex-provider-core/src/translators/gemini/request.rs` keeps request-body mutation and request module wiring
  - `crates/prodex-provider-core/src/translators/gemini/request/tool_signatures.rs` owns Gemini request tool-call thought-signature preservation
  - `crates/prodex-provider-core/src/translators/gemini/request_contents.rs` keeps Gemini request content module wiring and multimodal admission checks
  - `crates/prodex-provider-core/src/translators/gemini/request_contents/items.rs` owns Gemini contents shaping from Responses input items
  - `crates/prodex-provider-core/src/translators/gemini/request_contents/system_instruction.rs` owns Gemini systemInstruction shaping
  - `crates/prodex-provider-core/src/translators/gemini/request_contents/text.rs` owns text extraction and contextual-user instruction detection
  - `crates/prodex-provider-core/src/translators/gemini/request/continuation.rs` owns Gemini continuation metadata extraction
  - `crates/prodex-provider-core/src/translators/gemini/request/generation_config.rs` owns Gemini generation-config and text format mapping
  - `crates/prodex-provider-core/src/translators/gemini/request/generation_config/thinking.rs` owns Gemini thinking-config mapping
  - `crates/prodex-provider-core/src/translators/gemini/request/optional_fields.rs` owns Gemini optional request field passthrough
  - `crates/prodex-provider-core/src/translators/gemini/request/response_format.rs` owns Gemini response_format to generationConfig mapping
  - `crates/prodex-provider-core/src/translators/gemini/request/schema.rs` owns Gemini schema sanitization
  - `crates/prodex-provider-core/src/translators/gemini/request/schema/composition.rs` owns Gemini schema union/composition sanitization
  - `crates/prodex-provider-core/src/translators/gemini/request/tools.rs` owns Gemini tool declarations and tool-choice mapping
  - `crates/prodex-provider-core/src/translators/gemini/request/tools/builtin.rs` owns Gemini built-in request tool mapping
- Gemini provider translator response helpers are split by pure response responsibility:
  - `crates/prodex-provider-core/src/translators/gemini/response_transform.rs` owns buffered response transform orchestration
  - `crates/prodex-provider-core/src/translators/gemini/response.rs` keeps Responses output assembly orchestration
  - `crates/prodex-provider-core/src/translators/gemini/response/build.rs` owns GenerateContent response object assembly
  - `crates/prodex-provider-core/src/translators/gemini/response/chat_messages.rs` owns chat-compatible assistant message shaping
  - `crates/prodex-provider-core/src/translators/gemini/response/status.rs` owns finish-reason and status/error normalization
  - `crates/prodex-provider-core/src/translators/gemini/response/grounding.rs` owns citation and web-search response items
  - `crates/prodex-provider-core/src/translators/gemini/response/metadata.rs` owns usage and metadata normalization
  - `crates/prodex-provider-core/src/translators/gemini/response_tool_calls.rs` owns Responses tool-call item shaping
  - `crates/prodex-provider-core/src/translators/gemini/response_tool_calls/chat.rs` owns chat-compatible assistant tool-call item shaping
  - `crates/prodex-provider-core/src/translators/gemini/response_tool_calls_apply_patch.rs` owns Gemini apply-patch argument normalization
  - `crates/prodex-provider-core/src/translators/gemini/response_tool_calls_apply_patch/unified_diff.rs` owns unified-diff to apply_patch conversion
  - `crates/prodex-provider-core/src/translators/gemini/response_tool_calls/rtk.rs` owns RTK shell-command argument wrapping for Gemini response tool calls
  - `crates/prodex-provider-core/src/translators/gemini/response_tool_calls/rtk/noisy.rs` owns the Gemini-specific noisy command catalog
- Gemini provider translator stream helpers are split into `crates/prodex-provider-core/src/translators/gemini/stream.rs`, which owns SSE event framing and delta normalization.
- Copilot provider translator helpers are split by pure compatibility responsibility:
  - `crates/prodex-provider-core/src/translators/copilot.rs` keeps translator contract wiring
  - `crates/prodex-provider-core/src/translators/copilot/request.rs` owns canonical-model, encrypted-content, agent-input, and vision-input request helpers
  - `crates/prodex-provider-core/src/translators/copilot/response.rs` owns response-id extraction
  - `crates/prodex-provider-core/src/translators/copilot/tests.rs` owns Copilot characterization tests
- Shared OpenAI chat-compatible translator helpers are split by pure bridge responsibility:
  - `crates/prodex-provider-core/src/translators/openai_chat_compat.rs` keeps request-translation orchestration and module wiring
  - `crates/prodex-provider-core/src/translators/openai_chat_compat_params.rs` owns supported-parameter reporting
  - `crates/prodex-provider-core/src/translators/openai_chat_compat_request.rs` owns request message conversion
  - `crates/prodex-provider-core/src/translators/openai_chat_compat_request/validation.rs` owns request-shape validation
  - `crates/prodex-provider-core/src/translators/openai_chat_compat_request/validation/input_content.rs` owns Responses input content guards
  - `crates/prodex-provider-core/src/translators/openai_chat_compat_response.rs` owns buffered response conversion
  - `crates/prodex-provider-core/src/translators/openai_chat_compat_response/stream.rs` owns SSE stream event conversion
  - `crates/prodex-provider-core/src/translators/openai_chat_compat_util.rs` owns shared JSON/text/tool helpers
- Chat-compatible Responses tool helpers are split by pure tool-shape responsibility:
  - `crates/prodex-provider-core/src/chat_tools_bridge.rs` keeps public compatibility re-exports and characterization tests
  - `crates/prodex-provider-core/src/chat_tools_bridge/tools.rs` owns generic Responses function translation and tool helper orchestration
  - `crates/prodex-provider-core/src/chat_tools_bridge/tools/custom.rs` owns custom/freeform Responses tool shaping
  - `crates/prodex-provider-core/src/chat_tools_bridge/tools/mcp_toolset.rs` owns MCP toolset expansion
  - `crates/prodex-provider-core/src/chat_tools_bridge/tools/namespace.rs` owns namespace tool expansion
  - `crates/prodex-provider-core/src/chat_tools_bridge/tools/tool_search.rs` owns tool-search shaping
  - `crates/prodex-provider-core/src/chat_tools_bridge/tool_choice.rs` owns tool-choice mapping
  - `crates/prodex-provider-core/src/chat_tools_bridge/web_search.rs` owns web-search option extraction
  - `crates/prodex-provider-core/src/chat_tools_bridge/util.rs` owns shared JSON/name helpers
- Shared provider bridge helpers are split by pure bridge responsibility:
  - `crates/prodex-provider-core/src/bridge.rs` keeps public re-exports, rewritten-body helpers, and characterization tests
  - `crates/prodex-provider-core/src/bridge/chat_compat.rs` owns chat-compatible module wiring and request-shape validation
  - `crates/prodex-provider-core/src/bridge/chat_compat/response.rs` owns chat-compatible buffered response conversion
  - `crates/prodex-provider-core/src/bridge/chat_compat/tool_calls.rs` owns chat-compatible tool-call bridge helpers
  - `crates/prodex-provider-core/src/bridge/chat_compat/usage.rs` owns chat-compatible token usage mapping
- Anthropic dispatch is split by runtime responsibility:
  - `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream.rs` keeps shared request shaping and provider dispatch
  - `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_anthropic.rs` owns Anthropic URL/auth/model attempt ordering, translation selection, retry classification, pending-conversation cleanup, and response construction
- buffered chat-compatible response rewriting already prefers provider-core lossless/degraded output before falling back to app-side translation in:
  - `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite/response.rs`
- DeepSeek chat-compatible SSE runtime state is now split by responsibility:
  - `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse.rs` keeps chunk observation and provider-stream metadata updates while delegating fallback/upstream response ID selection plus chunk/choice metadata and delta extraction to provider-core
  - `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse/tool_calls.rs` owns streamed tool-call accumulation and event emission while delegating provider-specific delta field extraction, fallback IDs, delta/function-object validation, and argument validation to provider-core
  - `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse/completion.rs` owns final completion orchestration and conversation snapshot persistence while delegating stream error classification, response-created/completed event values, fallback/upstream response ID selection, stream chunk/choice metadata and delta extraction, response root value, stream metadata, output-text item IDs and item/delta values, streamed tool-call delta field extraction, streamed tool-call fallback IDs, streamed tool-call delta/function-object validation, streamed tool-call argument validation, streamed tool-call item/events, function-call argument delta source, and chat-assistant message shaping to provider-core
- Kiro provider translator helpers are split by pure compatibility responsibility:
  - `crates/prodex-provider-core/src/translators/kiro.rs` keeps translator contract wiring
  - `crates/prodex-provider-core/src/translators/kiro/acp.rs` owns Kiro ACP JSON-RPC request shaping, model-list item shaping, response-root/status shaping, assistant-output/chat-assistant message shaping, metadata shaping, plan-entry shaping, error/session-info shaping, stop-reason extraction, and incomplete-details mapping/value shaping
  - `crates/prodex-provider-core/src/translators/kiro/supported_params.rs` owns chat-completions supported-parameter reporting
  - `crates/prodex-provider-core/src/translators/kiro/request.rs` owns chat request orchestration
  - `crates/prodex-provider-core/src/translators/kiro/request/controls.rs` owns chat control validation and token-limit stripping
  - `crates/prodex-provider-core/src/translators/kiro/request/messages.rs` owns chat prompt/message and legacy function-tool shaping
  - `crates/prodex-provider-core/src/translators/kiro/request/messages/text.rs` owns chat content text extraction
  - `crates/prodex-provider-core/src/translators/kiro/response.rs` owns model endpoint response shaping, Kiro error body shaping, response runtime metadata decoration, response tool-call/finish-reason detection, chat response compatibility rewriting, and Anthropic message response shaping
  - `crates/prodex-provider-core/src/translators/kiro/stream.rs` owns chat-completion stream chunk/delta/terminal-delta shaping, Responses stream event shaping, stream tool-call argument/delta bridge shaping, stream content text extraction, stream tool-call item shaping, Kiro ACP tool-call item shaping, and Kiro ACP usage-update shaping
  - `crates/prodex-provider-core/src/translators/kiro/compact.rs` owns semantic compact helpers
  - `crates/prodex-provider-core/src/translators/kiro/tests.rs` owns Kiro characterization tests
  - `crates/prodex-provider-core/src/translators/kiro/request_tests.rs` owns request-control, prompt, legacy-function, and tool-choice parity tests
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro.rs` now keeps only ACP process/session lifecycle, stream ordering, transport assembly, persistence, auth staging, and runtime metadata orchestration; pure protocol and body shaping is delegated to provider-core.

So the remaining duplication is narrower than before, but not done:

- request-history extraction for pending continuation state still lives in runtime
- SSE readers and stream event ordering still live in runtime
- Gemini and Kiro still have substantial app-owned request/response shaping
- chat-compatible fallback translators still exist in app code

### 2. Which transforms belong in `prodex-provider-core`

Keep in provider core:

- request body translation
- response body translation
- SSE / stream event normalization
- explicit capability / supported-parameter declarations
- degraded / rejected / unsupported decisions
- provider error classification
  - `crates/prodex-provider-core/src/errors.rs` owns canonical provider error classification
  - `crates/prodex-provider-core/src/errors/body.rs` owns provider error-body token extraction
- usage extraction
- provider model fallback metadata and model body rewrite helpers:
  - `crates/prodex-provider-core/src/fallback.rs` keeps compatibility re-exports and characterization tests
  - `crates/prodex-provider-core/src/fallback/body.rs` owns request-body model extraction/rewrite
  - `crates/prodex-provider-core/src/fallback/chains.rs` owns provider fallback-chain and canonical-model policy
  - `crates/prodex-provider-core/src/fallback/chains/gemini.rs` owns Gemini fallback aliases and Code Assist model filtering

Move or finish moving from app/runtime into provider core:

- Gemini request orchestration still has app-owned callers, while pure request-shape bridge helpers now live in `crates/prodex-provider-core/src/gemini_bridge/request.rs` and `crates/prodex-provider-core/src/gemini_bridge/request/tools.rs`:
  - `runtime_launch/proxy_startup/gemini_request.rs`
  - `runtime_launch/proxy_startup/gemini_request_*`
- Gemini response normalization still owned by:
  - `crates/prodex-provider-core/src/translators/gemini/response.rs`
  - `runtime_launch/proxy_startup/gemini_sse.rs`
  - `runtime_launch/proxy_startup/gemini_sse_state.rs`
  - `runtime_launch/proxy_startup/gemini_sse_state/complete.rs` (final response, completion guardrails, and conversation binding)
  - `runtime_launch/proxy_startup/gemini_sse_state/events.rs` (text/reasoning/media/citation SSE event emission helpers)
  - `runtime_launch/proxy_startup/gemini_sse_state/output.rs` (output/conversation assembly split; stream state machine remains app-owned)
- DeepSeek request/response/SSE normalization still duplicated through:
  - `runtime_launch/proxy_startup/deepseek_rewrite.rs`
  - `runtime_launch/proxy_startup/deepseek_rewrite/*`
  - `runtime_launch/proxy_startup/deepseek_sse.rs`
  - `runtime_launch/proxy_startup/deepseek_sse/tool_calls.rs`
  - `runtime_launch/proxy_startup/deepseek_sse/completion.rs`
- shared OpenAI-chat-compatible request/response fallback logic still owned by app runtime:
  - `runtime_launch/proxy_startup/chat_compatible_request.rs`
  - `runtime_launch/proxy_startup/local_rewrite_response_chat_compatible.rs`
- Kiro protocol/body rewrite logic that does not depend on transport/session state lives in:
  - `crates/prodex-provider-core/src/translators/kiro.rs`
  - `crates/prodex-provider-core/src/translators/kiro/request.rs`
  - `crates/prodex-provider-core/src/translators/kiro/response.rs`
  - `crates/prodex-provider-core/src/translators/kiro/compact.rs`
  - app runtime transport and session state remain in `runtime_launch/proxy_startup/local_rewrite_kiro.rs`

Do **not** move further into provider core unless the code is pure and state-free:

- Copilot `responses` native passthrough handling
- auth/header decoration
- pending-message storage
- profile affinity / request rotation

Keep out of provider core:

- auth pools
- profile rotation
- websocket lifecycle
- retry timing
- continuation ownership
- gateway admission/state mutation

### 3. Runtime and gateway paths that still know too much about providers

Provider knowledge is still concentrated in `prodex-app` runtime launch paths:

- `runtime_launch/proxy_startup/*gemini*`
- `runtime_launch/proxy_startup/*deepseek*`
- `runtime_launch/proxy_startup/*kiro*`
- `runtime_launch/proxy_startup/provider_bridge.rs`
- `runtime_launch/proxy_startup/local_rewrite_response_chat_compatible.rs`
- `runtime_launch/proxy_startup/local_rewrite_upstream.rs`
- `runtime_launch/proxy_startup/local_rewrite_anthropic.rs`
- `app_commands/runtime_launch/providers.rs`
- `app_commands/runtime_launch/gateway_provider_config.rs`
- `app_commands/runtime_launch/gateway_config.rs`

This means provider behavior is not yet fully centralized even though the core contract exists.

### 4. App-server / exec-server / MCP status

Current app-server broker status is still passthrough-first, with a real diagnostic scaffold:

- `crates/prodex-app/src/app_server_broker.rs`
- `crates/prodex-app/src/app_commands/app_server_broker.rs`
- `crates/prodex-app/src/app_server_broker_preview.rs`
- `crates/prodex-app/src/app_server_broker_preview/logging.rs`
- `crates/prodex-app/src/app_server_broker_preview/report.rs`
- `crates/prodex-app/src/app_commands/runtime_launch.rs`
- `crates/prodex-app/src/runtime_launch/routes.rs`

Current state:

- lifecycle method parsing exists
- JSON-RPC envelope metadata extraction now exists for `session_id`, `thread_id`, `turn_id`, and `item_id`
- stateless lifecycle method/frame schema hints now map request/notification frames to imported upstream Codex schema fixture files
- contract JSON exists
- default mode is still `"direct-passthrough"`
- experimental stdio preview mode exists as a read-only line-by-line diagnostic stream
- experimental passthrough-aware stdio preview exists as a read-only mirror plus side-channel diagnostics stream
- experimental stdio validate mode exists as a read-only fail-closed protocol gate for replay/schema streams
- opt-in live stdio brokering launches the real `codex app-server`, relays both
  directions, validates bounded JSON-RPC frames, and fails closed on malformed
  traffic

So Phase 2 now includes diagnostic envelope parsing, preview streaming,
passthrough-aware observation, fail-closed validation, and an opt-in live broker.
Cross-session multiplexing and broker-owned routing policy remain future work.

`mcp-server`, `app-server`, and `exec-server` launch routes remain direct
passthrough by default. The opt-in broker enforces the protocol contract but
does not invent broker-owned routing policy.

### 5. Where gateway state / auth / budgets / routing / audit are concentrated

Gateway/control-plane behavior exists, but is app-heavy and concentrated:

- `crates/prodex-app/src/app_commands/runtime_launch/gateway_config.rs`
- `crates/prodex-app/src/app_commands/runtime_launch/gateway_auth_config.rs`
- `crates/prodex-app/src/app_commands/runtime_launch/gateway_guardrail_config.rs`
- `crates/prodex-app/src/app_commands/runtime_launch/gateway_observability_config.rs`
- `crates/prodex-app/src/app_commands/runtime_launch/gateway_provider_config.rs`
- `crates/prodex-app/src/app_commands/runtime_launch/gateway_route_alias_config.rs`
- `crates/prodex-app/src/app_commands/runtime_launch/gateway_sso_config.rs`
- `crates/prodex-app/src/app_commands/runtime_launch/gateway_state_store_config.rs`
- `crates/prodex-app/src/app_commands/runtime_launch.rs`
- `crates/prodex-app/src/app_commands/gateway.rs`
- runtime proxy / gateway config types from `runtime_proxy_crate`

What already exists:

- auth token config
- admin tokens
- virtual key config
- route aliases
- guardrails
- state-store selection (`file` / `sqlite`)
- observability config
- provider / upstream resolution
- SSO / OIDC config
- audit log crate and hooks

What is still weak structurally:

- gateway launch wiring still centralizes final assembly in `gateway_config.rs`, but the policy parsing layer is no longer a single monolith
- hot-path gateway decisions are not yet clearly split into auth / budget / router / audit / hooks modules
- no checked-in repository abstraction boundary for state backends
- no obvious LiteLLM-style callback layer in app-facing code
- runtime proxy observability is structured, but request/session/turn/provider tracing is still log-centric rather than OTEL-style span-centric

Current gateway launch split status:

- `gateway_config.rs` is now mostly orchestration/wiring
- extracted policy/config seams now exist for:
  - auth + virtual keys
  - guardrails + webhook policy
  - observability sinks
  - provider/upstream selection
  - route aliases + cost inference
  - SSO/OIDC
  - state store backend selection
- runtime local-rewrite transport now has focused submodules for:
  - auth attempt ordering and transport-local header filtering
  - gateway spend observability sinks
  - provider upstream URL mapping
- runtime gateway policy now has a focused routing submodule:
  - `crates/prodex-runtime-proxy/src/gateway_policy.rs` keeps public gateway-policy exports and characterization tests
  - `crates/prodex-runtime-proxy/src/gateway_policy/routing.rs` owns route alias strategies and model rewrite decisions
  - `crates/prodex-runtime-proxy/src/gateway_policy/virtual_keys.rs` owns virtual-key auth, admission, limits, and usage bookkeeping

### 6. Largest hot-path files that should be split first

Current large files from the checked worktree include:

- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests.rs` (~3153; tests, not runtime hot path)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/deepseek.rs` (~2390; tests, not runtime hot path)
- `crates/prodex-provider-core/src/gemini_bridge/tests.rs` (~2048; characterization tests, not runtime hot path)
- `crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge/tests/` (split characterization tests, not runtime hot path)
- `crates/prodex-provider-core/src/deepseek_bridge/tests.rs` (~1163; characterization tests, not runtime hot path)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_oauth_pool.rs` (~415; OAuth auth selection orchestration)
- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_provider_tool_tests.rs` (~707; tests, not runtime hot path)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_rewrite_tests.rs` (~699; tests, not runtime hot path)
- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite_tests.rs` (~653; tests, not runtime hot path)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_tests.rs` (~554; tests, not runtime hot path)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_tests.rs` (~544; tests, not runtime hot path)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_openai_compat_tests.rs` (~504; tests, not runtime hot path)

Recent split or verified-below-threshold files:

- `crates/prodex-app/src/app_server_broker_protocol.rs` (~356; protocol summary/types after report, wire, affinity, lifecycle, and metadata splits)
- `crates/prodex-app/src/app_server_broker_protocol/affinity.rs` (~256; continuation affinity and safe-rotation policy decisions)
- `crates/prodex-app/src/app_server_broker_protocol/lifecycle.rs` (~169; lifecycle method, stage, and schema hint mapping)
- `crates/prodex-app/src/app_server_broker_protocol/metadata.rs` (~113; session/thread/turn/item metadata extraction)
- `crates/prodex-app/src/app_server_broker_protocol/report.rs` (~162; broker diagnostic/request/response JSON report rendering)
- `crates/prodex-app/src/app_server_broker_protocol/wire.rs` (~156; JSON-RPC wire-frame classification and invalid-reason mapping)
- `crates/prodex-provider-core/src/lib.rs` (~243; provider-core module wiring and public re-exports after surface split)
- `crates/prodex-provider-core/src/translator.rs` (~50; translator trait and public re-exports after DTO split)
- `crates/prodex-provider-core/src/translator/transform.rs` (~146; transform input/result DTOs after status-label split)
- `crates/prodex-provider-core/src/translator/transform/status.rs` (~72; canonical transform status/loss/outcome labels)
- `crates/prodex-provider-core/src/translator/conformance.rs` (~79; provider conformance fixture DTOs)
- `crates/prodex-provider-core/src/translator/params.rs` (~24; provider parameter support DTOs)
- `crates/prodex-provider-core/src/surface.rs` (~155; provider identifiers and capability labels after surface contract split)
- `crates/prodex-provider-core/src/surface/adapter_contract.rs` (~99; provider adapter contract and body transform wrappers)
- `crates/prodex-provider-core/src/surface/endpoints.rs` (~71; provider endpoint lists and lookup helper)
- `crates/prodex-provider-core/src/surface/models.rs` (~50; provider model surface metadata types)
- `crates/prodex-app/src/runtime_launch/proxy_startup.rs` (~485; startup orchestration after worker-spawn split)
- `crates/prodex-app/src/runtime_launch/proxy_startup/workers.rs` (~134; runtime rotation proxy worker spawning)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_openapi_paths.rs` (~435; gateway OpenAPI path orchestration after billing path split)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_openapi_paths/billing.rs` (~96; billing ledger OpenAPI path definitions)
- `crates/prodex-app/src/runtime_proxy/smart_context/runtime_rehydrate/appendix.rs` (~328; appendix rendering after semantic candidate selection split)
- `crates/prodex-app/src/runtime_proxy/smart_context/runtime_rehydrate/appendix/selection.rs` (~202; semantic candidate selection and allocation metadata)
- `crates/prodex-app/src/runtime_proxy/smart_context/body.rs` (~439; smart-context body orchestration after proxy-state transform split)
- `crates/prodex-app/src/runtime_proxy/smart_context/body/transform.rs` (~96; proxy-state JSON body transform steps)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_compact.rs` (~417; compact wrapper delegates summary shaping to provider-core)
- `crates/prodex-app/src/runtime_launch/proxy_startup/provider_tools.rs` (~294; small provider-tool wrapper, no longer a primary split target)
- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse.rs` (~281; chunk observation after split)
- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse/completion.rs` (~290; completion/output assembly)
- `crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse/tool_calls.rs` (~272; streamed tool-call accumulation)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport.rs` (~257; send path after split)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport/observability.rs` (~167; gateway spend sinks)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream.rs` (~572; shared provider dispatch/upstream send orchestration)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_anthropic.rs` (~632; Anthropic auth/model attempt state machine and response construction)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream_embeddings.rs` (~146; deterministic local embeddings fallback)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_copilot.rs` (~379; Copilot provider dispatch/request shaping after state split)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_copilot/auth.rs` (~144; Copilot auth attempt ordering and response affinity)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_copilot/state.rs` (~94; Copilot provider auth state, profile catalog, and OAuth pool construction)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_context.rs` (~244; local context collection orchestration)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_context/filter.rs` (~414; gitignore/geminiignore filtering)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_context/path_match.rs` (~89; glob/path matching)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live.rs` (~309; sidecar/listener and request handling)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live/connection.rs` (~69; upstream connect and local upgrade helpers)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live/session.rs` (~244; websocket session pumps)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_oauth_pool/quota.rs` (~100; quota refresh worker and cached headers)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_oauth_pool/state.rs` (~280; OAuth affinity/model state bookkeeping)
- `crates/prodex-provider-core/src/translators/openai_chat_compat.rs` (~139; shared request orchestration and module wiring after util split)
- `crates/prodex-provider-core/src/translators/openai_chat_compat_params.rs` (~124; chat-compatible supported-parameter reporting)
- `crates/prodex-provider-core/src/translators/openai_chat_compat_request.rs` (~158; request message conversion after validation split)
- `crates/prodex-provider-core/src/translators/openai_chat_compat_request/validation.rs` (~140; request-shape validation after input-content split)
- `crates/prodex-provider-core/src/translators/openai_chat_compat_request/validation/input_content.rs` (~65; Responses input content guards)
- `crates/prodex-provider-core/src/translators/openai_chat_compat_response.rs` (~123; buffered chat response conversion after stream split)
- `crates/prodex-provider-core/src/translators/openai_chat_compat_response/stream.rs` (~147; chat stream event conversion)
- `crates/prodex-provider-core/src/translators/openai_chat_compat_util.rs` (~147; shared JSON/text/tool helpers)
- `crates/prodex-provider-core/src/translators/copilot.rs` (~103; Copilot translator contract wiring after test split)
- `crates/prodex-provider-core/src/translators/copilot/request.rs` (~124; canonical-model, encrypted-content, agent-input, and vision-input helpers)
- `crates/prodex-provider-core/src/translators/copilot/response.rs` (~15; response-id extraction)
- `crates/prodex-provider-core/src/translators/copilot/tests.rs` (~83; Copilot characterization tests)
- `crates/prodex-provider-core/src/translators/kiro.rs` (~230; Kiro translator contract wiring after ACP split)
- `crates/prodex-provider-core/src/translators/kiro/acp.rs` (~199; Kiro ACP model-list item shaping, response-root/status shaping, assistant-output/chat-assistant message shaping, metadata shaping, plan-entry shaping, error/session-info shaping, stop-reason extraction, and incomplete-details mapping/value shaping)
- `crates/prodex-provider-core/src/translators/kiro/supported_params.rs` (~60; Kiro chat-completions supported-parameter reporting)
- `crates/prodex-provider-core/src/translators/kiro/tests.rs` (~459; Kiro characterization tests)
- `crates/prodex-provider-core/src/translators/kiro/request.rs` (~184; Kiro chat request orchestration after controls/message splits)
- `crates/prodex-provider-core/src/translators/kiro/request/controls.rs` (~68; Kiro supported-parameter checks and token-limit stripping)
- `crates/prodex-provider-core/src/translators/kiro/request/messages.rs` (~146; Kiro chat message and legacy function-tool shaping after text split)
- `crates/prodex-provider-core/src/translators/kiro/request/messages/text.rs` (~36; Kiro chat content text extraction)
- `crates/prodex-provider-core/src/translators/kiro/response.rs` (~117; Kiro chat response compatibility rewrite)
- `crates/prodex-provider-core/src/translators/kiro/stream.rs` (~161; Kiro chat-completion stream chunk shaping, stream content text extraction, stream tool-call item shaping, Kiro ACP tool-call item shaping, and Kiro ACP usage-update shaping)
- `crates/prodex-provider-core/src/translators/kiro/compact.rs` (~70; Kiro semantic compact helpers)
- `crates/prodex-provider-core/src/translators/deepseek.rs` (~63; DeepSeek translator contract wiring after request-transform split)
- `crates/prodex-provider-core/src/translators/deepseek/request_transform.rs` (~114; DeepSeek request translation)
- `crates/prodex-provider-core/src/translators/deepseek/request.rs` (~123; DeepSeek chat request body assembly after params split)
- `crates/prodex-provider-core/src/translators/deepseek/request/params.rs` (~98; DeepSeek request parameter validation and mapping)
- `crates/prodex-provider-core/src/translators/deepseek/response_transform.rs` (~53; DeepSeek buffered response translation)
- `crates/prodex-provider-core/src/translators/deepseek/response.rs` (~123; DeepSeek buffered response assembly after metadata split)
- `crates/prodex-provider-core/src/translators/deepseek/response/metadata.rs` (~63; DeepSeek buffered response metadata extraction)
- `crates/prodex-provider-core/src/translators/deepseek/stream.rs` (~84; DeepSeek stream event translation)
- `crates/prodex-provider-core/src/translators/deepseek/supported_params.rs` (~57; DeepSeek supported-parameter reporting)
- `crates/prodex-provider-core/src/translators/deepseek/tooling.rs` (~33; DeepSeek tooling re-export module)
- `crates/prodex-provider-core/src/translators/deepseek/tooling/messages.rs` (~125; request input/message orchestration after chat-item split)
- `crates/prodex-provider-core/src/translators/deepseek/tooling/messages/chat_items.rs` (~64; generic chat content and tool-call shaping)
- `crates/prodex-provider-core/src/translators/deepseek/tooling/messages/input_tool_calls.rs` (~138; Responses input tool-call item shaping after local-shell split)
- `crates/prodex-provider-core/src/translators/deepseek/tooling/messages/local_shell.rs` (~61; local-shell input call shaping)
- `crates/prodex-provider-core/src/translators/deepseek/tooling/messages/thought_signature.rs` (~44; DeepSeek tool-call thought-signature extraction)
- `crates/prodex-provider-core/src/translators/deepseek/tooling/response_tool_calls.rs` (~104; response tool-call normalization)
- `crates/prodex-provider-core/src/models.rs` (~66; provider model catalog lookup/macro)
- `crates/prodex-provider-core/src/models/openai.rs` (~69; OpenAI model catalog)
- `crates/prodex-provider-core/src/models/anthropic.rs` (~117; Anthropic model catalog)
- `crates/prodex-provider-core/src/models/copilot.rs` (~5; Copilot model catalog module boundary)
- `crates/prodex-provider-core/src/models/copilot/catalog.rs` (~312; Copilot model catalog data)
- `crates/prodex-provider-core/src/models/deepseek.rs` (~57; DeepSeek model catalog)
- `crates/prodex-provider-core/src/models/gemini.rs` (~5; Gemini model catalog module boundary)
- `crates/prodex-provider-core/src/models/gemini/catalog.rs` (~237; Gemini model catalog data)
- `crates/prodex-provider-core/src/models/kiro.rs` (~33; Kiro model catalog)
- `crates/prodex-provider-core/src/models/local.rs` (~18; local model catalog)
- `crates/prodex-provider-core/src/contract.rs` (~169; provider contract matrix after markdown split)
- `crates/prodex-provider-core/src/contract/markdown.rs` (~116; provider capability Markdown rendering)
- `crates/prodex-provider-core/src/translators/gemini.rs` (~129; Gemini translator trait wiring after request/response/stream splits)
- `crates/prodex-provider-core/src/translators/gemini/request_transform.rs` (~138; Gemini request transform orchestration after continuation split)
- `crates/prodex-provider-core/src/translators/gemini/request.rs` (~61; request body mutation and module wiring after continuation split)
- `crates/prodex-provider-core/src/translators/gemini/request/tool_signatures.rs` (~126; request tool-call thought-signature preservation)
- `crates/prodex-provider-core/src/translators/gemini/request_contents.rs` (~29; request content module wiring and multimodal admission checks)
- `crates/prodex-provider-core/src/translators/gemini/request_contents/items.rs` (~129; Gemini contents shaping from Responses input items)
- `crates/prodex-provider-core/src/translators/gemini/request_contents/system_instruction.rs` (~28; Gemini systemInstruction shaping)
- `crates/prodex-provider-core/src/translators/gemini/request_contents/text.rs` (~62; text extraction and contextual-user instruction detection)
- `crates/prodex-provider-core/src/translators/gemini/request/continuation.rs` (~24; continuation metadata extraction)
- `crates/prodex-provider-core/src/translators/gemini/request/generation_config.rs` (~140; generation-config and text-format mapping after thinking-config split)
- `crates/prodex-provider-core/src/translators/gemini/request/generation_config/thinking.rs` (~64; thinking-config mapping)
- `crates/prodex-provider-core/src/translators/gemini/request/optional_fields.rs` (~25; optional request field passthrough)
- `crates/prodex-provider-core/src/translators/gemini/request/response_format.rs` (~39; response_format to generationConfig mapping)
- `crates/prodex-provider-core/src/translators/gemini/request/schema.rs` (~140; schema sanitization after composition split)
- `crates/prodex-provider-core/src/translators/gemini/request/schema/composition.rs` (~46; schema union/composition sanitization)
- `crates/prodex-provider-core/src/translators/gemini/request/tools.rs` (~77; tool declarations and tool-choice mapping after built-in split)
- `crates/prodex-provider-core/src/translators/gemini/request/tools/builtin.rs` (~86; Gemini built-in request tool mapping)
- `crates/prodex-provider-core/src/gemini_bridge/request/simple.rs` (~124; Gemini simple-request item/content fast-path after built-in split)
- `crates/prodex-provider-core/src/gemini_bridge/request/simple/builtin.rs` (~34; Gemini simple-request built-in tool classification)
- `crates/prodex-provider-core/src/gemini_bridge/compact.rs` (~15; compact module wiring after semantic/local split)
- `crates/prodex-provider-core/src/gemini_bridge/compact/semantic.rs` (~118; semantic compact request and summary helpers)
- `crates/prodex-provider-core/src/gemini_bridge/compact/local.rs` (~85; local compact fallback summary/body after semantic split)
- `crates/prodex-provider-core/src/gemini_bridge/compact/local/semantic.rs` (~87; semantic compact continuation summary)
- `crates/prodex-provider-core/src/gemini_bridge/compact/local/snippet.rs` (~86; local compact message/reasoning snippet extraction after tool split)
- `crates/prodex-provider-core/src/gemini_bridge/compact/local/snippet/tool.rs` (~104; local compact tool snippet extraction)
- `crates/prodex-provider-core/src/gemini_bridge/request.rs` (~107; Gemini request bridge body/map helpers after tool split)
- `crates/prodex-provider-core/src/gemini_bridge/request/tools.rs` (~89; Gemini request bridge tool/schema helpers)
- `crates/prodex-provider-core/src/gemini_bridge/media.rs` (~124; Gemini media orchestration after content-object/mime split)
- `crates/prodex-provider-core/src/gemini_bridge/media/content_object.rs` (~80; Gemini media content-object conversion)
- `crates/prodex-provider-core/src/gemini_bridge/media/mime.rs` (~60; Gemini media MIME helpers)
- `crates/prodex-provider-core/src/gemini_bridge/live/audio.rs` (~118; Gemini Live audio configuration after codec/payload split)
- `crates/prodex-provider-core/src/gemini_bridge/live/audio/codec.rs` (~31; Gemini Live G.711 codec helpers)
- `crates/prodex-provider-core/src/gemini_bridge/live/audio/payload.rs` (~48; Gemini Live audio payload conversion)
- `crates/prodex-provider-core/src/gemini_bridge/live/events.rs` (~100; Gemini Live event builders after error/output split)
- `crates/prodex-provider-core/src/gemini_bridge/live/events/error.rs` (~40; Gemini Live error event builders)
- `crates/prodex-provider-core/src/gemini_bridge/live/events/output.rs` (~91; Gemini Live output and transcript event builders)
- `crates/prodex-provider-core/src/gemini_bridge/live/messages.rs` (~86; Gemini Live client/content/tool/audio messages after setup split)
- `crates/prodex-provider-core/src/gemini_bridge/live/messages/setup.rs` (~80; Gemini Live setup message builders)
- `crates/prodex-provider-core/src/gemini_bridge/precommit.rs` (~97; Gemini stream pre-commit decision flow after output split)
- `crates/prodex-provider-core/src/gemini_bridge/precommit/output.rs` (~74; Gemini stream pre-commit output detection)
- `crates/prodex-provider-core/src/gemini_bridge/tool_io.rs` (~118; Gemini function call/response shaping after command-response/mask split)
- `crates/prodex-provider-core/src/gemini_bridge/tool_io/command_response.rs` (~128; structured command tool response parsing)
- `crates/prodex-provider-core/src/gemini_bridge/tool_io/mask.rs` (~76; Gemini tool-output history masking)
- `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails.rs` (~100; Gemini assistant/tool-output guardrails after intent split)
- `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails/intent.rs` (~67; Gemini assistant tool-intent guardrail)
- `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails/exact_output/commands.rs` (~65; exact-output command marker extraction after matching split)
- `crates/prodex-provider-core/src/gemini_bridge/tooling/guardrails/exact_output/commands/matching.rs` (~94; exact-output command matching against assistant tool calls)
- `crates/prodex-provider-core/src/gemini_bridge/errors/quota_retry.rs` (~97; quota/retry classifiers after body split)
- `crates/prodex-provider-core/src/gemini_bridge/errors/quota_retry/body.rs` (~72; quota retry response-body parsing)
- `crates/prodex-provider-core/src/gemini_bridge/errors/quota_retry/retry_delay.rs` (~135; retry-delay JSON crawl and policy after duration split)
- `crates/prodex-provider-core/src/gemini_bridge/errors/quota_retry/retry_delay/duration.rs` (~74; retry-delay duration and header parsing)
- `crates/prodex-provider-core/src/translators/gemini/response_transform.rs` (~52; buffered response transform orchestration)
- `crates/prodex-provider-core/src/translators/gemini/response.rs` (~116; Responses output assembly orchestration after builder/chat-message split)
- `crates/prodex-provider-core/src/translators/gemini/response/build.rs` (~155; GenerateContent response object assembly)
- `crates/prodex-provider-core/src/translators/gemini/response/chat_messages.rs` (~117; chat-compatible assistant message shaping)
- `crates/prodex-provider-core/src/translators/gemini/response/grounding.rs` (~159; citations and web-search items)
- `crates/prodex-provider-core/src/translators/gemini/response/status.rs` (~91; finish reason/status normalization)
- `crates/prodex-provider-core/src/translators/gemini/response/metadata.rs` (~77; usage and metadata normalization)
- `crates/prodex-provider-core/src/translators/gemini/response_tool_calls.rs` (~169; Gemini Responses tool-call item shaping after chat/RTK split)
- `crates/prodex-provider-core/src/translators/gemini/response_tool_calls/chat.rs` (~45; Gemini chat-compatible assistant tool-call item shaping)
- `crates/prodex-provider-core/src/gemini_bridge/response.rs` (~132; Gemini response bridge orchestration after simple/status/tool-call split)
- `crates/prodex-provider-core/src/gemini_bridge/response/status.rs` (~40; Gemini response bridge status and finish-reason wrappers)
- `crates/prodex-provider-core/src/gemini_bridge/response/tool_calls.rs` (~69; Gemini response bridge tool-call wrappers)
- `crates/prodex-provider-core/src/translators/gemini/response_tool_calls_apply_patch.rs` (~129; apply_patch argument normalization after unified-diff split)
- `crates/prodex-provider-core/src/translators/gemini/response_tool_calls_apply_patch/unified_diff.rs` (~166; unified-diff to apply_patch conversion)
- `crates/prodex-provider-core/src/translators/gemini/response_tool_calls/rtk.rs` (~134; RTK shell-command argument wrapping after catalog split)
- `crates/prodex-provider-core/src/translators/gemini/response_tool_calls/rtk/noisy.rs` (~73; Gemini-specific RTK noisy command catalog)
- `crates/prodex-provider-core/src/translators/tool_args.rs` (~37; generic JSON string-argument rewriting after RTK split)
- `crates/prodex-provider-core/src/translators/tool_args/rtk.rs` (~57; RTK JSON argument wrapping after shell insertion split)
- `crates/prodex-provider-core/src/translators/tool_args/rtk/shell.rs` (~125; RTK shell-command insertion helpers)
- `crates/prodex-provider-core/src/translators/tool_args/rtk_noisy.rs` (~73; RTK noisy shell command catalog)
- `crates/prodex-provider-core/src/translators/gemini/stream.rs` (~114; SSE event framing and delta normalization)
- `crates/prodex-provider-core/src/deepseek_bridge/input_items/push.rs` (~130; DeepSeek input item to chat message push orchestration after field split)
- `crates/prodex-provider-core/src/deepseek_bridge/input_items/push/fields.rs` (~66; DeepSeek input item field extraction)
- `crates/prodex-provider-core/src/deepseek_bridge/input_items/validation.rs` (~114; DeepSeek input-item role/tool validation after content split)
- `crates/prodex-provider-core/src/deepseek_bridge/input_items/validation/content.rs` (~97; DeepSeek message content validation)
- `crates/prodex-provider-core/src/deepseek_bridge/request_probe.rs` (~92; DeepSeek simple-request probe orchestration after input-item split)
- `crates/prodex-provider-core/src/deepseek_bridge/request_probe/input.rs` (~126; DeepSeek simple-request input-item eligibility checks)
- `crates/prodex-provider-core/src/deepseek_bridge/request_tools.rs` (~100; DeepSeek tool validation orchestration after function-tool/tool-shape split)
- `crates/prodex-provider-core/src/deepseek_bridge/request_tools/function_tools.rs` (~72; DeepSeek function-tool name and parameter validation)
- `crates/prodex-provider-core/src/deepseek_bridge/request_tools/shape.rs` (~114; DeepSeek primitive/custom/namespace tool-shape validation after MCP split)
- `crates/prodex-provider-core/src/deepseek_bridge/request_tools/shape/mcp.rs` (~88; DeepSeek MCP function/toolset validation)
- `crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_shape.rs` (~120; DeepSeek top-level tools-array shape validation)
- `crates/prodex-provider-core/src/deepseek_bridge/request_tools/tool_choice.rs` (~89; DeepSeek tool-choice validation)
- `crates/prodex-provider-core/src/deepseek_bridge/request_tools/web_search.rs` (~88; DeepSeek web-search option validation)
- `crates/prodex-provider-core/src/chat_tools_bridge.rs` (~132; public re-exports and characterization tests)
- `crates/prodex-provider-core/src/chat_tools_bridge/tools.rs` (~110; generic Responses function translation and orchestration after custom/MCP/namespace/tool-search split)
- `crates/prodex-provider-core/src/chat_tools_bridge/tools/custom.rs` (~50; custom/freeform Responses tool shaping)
- `crates/prodex-provider-core/src/chat_tools_bridge/tools/mcp_toolset.rs` (~93; MCP toolset expansion)
- `crates/prodex-provider-core/src/chat_tools_bridge/tools/namespace.rs` (~70; namespace tool expansion)
- `crates/prodex-provider-core/src/chat_tools_bridge/tools/tool_search.rs` (~30; tool-search shaping)
- `crates/prodex-provider-core/src/chat_tools_bridge/tool_choice.rs` (~63; tool-choice mapping)
- `crates/prodex-provider-core/src/chat_tools_bridge/web_search.rs` (~51; web-search option extraction)
- `crates/prodex-provider-core/src/chat_tools_bridge/util.rs` (~52; shared JSON/name helpers)
- `crates/prodex-provider-core/src/bridge.rs` (~285; public bridge re-exports, body helpers, characterization tests)
- `crates/prodex-provider-core/src/bridge/chat_compat.rs` (~62; chat-compatible module wiring and request-shape validation after response/usage/tool-call split)
- `crates/prodex-provider-core/src/bridge/chat_compat/response.rs` (~152; chat-compatible buffered response conversion)
- `crates/prodex-provider-core/src/bridge/chat_compat/tool_calls.rs` (~144; chat-compatible tool-call bridge helpers)
- `crates/prodex-provider-core/src/bridge/chat_compat/usage.rs` (~53; chat-compatible token usage mapping)
- `crates/prodex-provider-core/src/errors.rs` (~136; provider error classification after body-token split)
- `crates/prodex-provider-core/src/errors/body.rs` (~58; provider error body token extraction)
- `crates/prodex-provider-core/src/usage.rs` (~144; provider usage extraction and cost helpers after estimator split)
- `crates/prodex-provider-core/src/usage/estimate.rs` (~68; provider request token estimation)
- `crates/prodex-provider-core/src/fallback/chains.rs` (~112; provider fallback-chain and canonical-model policy after Gemini split)
- `crates/prodex-provider-core/src/fallback/chains/gemini.rs` (~88; Gemini fallback aliases and Code Assist filtering)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_send.rs` (~499; Gemini send/retry orchestration after observability-log extraction)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_send_logs.rs` (~268; Gemini send structured runtime log helpers)
- `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_send_model_chain.rs` (~106; Gemini auth/profile model-chain selection and sticky model preference)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_state.rs` (~411; Gemini stream observation state after completion split)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_state/complete.rs` (~126; final response, completion guardrails, and binding recorder)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_state/events.rs` (~231)
- `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_state/output.rs` (~182)
- `crates/prodex-app/src/runtime_proxy/standard/attempts/models.rs` (~214; standard-route OpenAI models metadata forwarding and Spark catalog patch)
- `crates/prodex-app/src/runtime_proxy/standard/attempts.rs` (~24; standard attempt module wiring after route-specific orchestration splits)
- `crates/prodex-app/src/runtime_proxy/standard/attempts/noncompact.rs` (~296; non-compact standard upstream attempt orchestration)
- `crates/prodex-app/src/runtime_proxy/standard/attempts/compact.rs` (~201; compact upstream attempt orchestration)
- `crates/prodex-app/src/runtime_proxy/standard/attempts/precommit.rs` (~114; standard/compact local pre-commit quota guards)
- `crates/prodex-app/src/runtime_proxy/standard/attempts/success.rs` (~83; compact/standard success forwarding and lineage recording)
- `crates/prodex-app/src/runtime_proxy/standard/compact/admission.rs` (~68; compact pressure/admission logging helpers)
- `crates/prodex-app/src/runtime_proxy/standard/compact/affinity.rs` (~20; compact hard-affinity predicate wrapper)
- `crates/prodex-app/src/runtime_proxy/standard/compact/logging.rs` (~102; compact final-failure reason and log construction helpers)
- `crates/prodex-app/src/runtime_proxy/standard/compact.rs` (~406; compact selection loop after retryable and transport failure splits)
- `crates/prodex-app/src/runtime_proxy/standard/compact/retryable.rs` (~235; compact quota/overload retry handling)
- `crates/prodex-app/src/runtime_proxy/standard/compact/transport.rs` (~70; compact transport failure handling)
- `crates/prodex-app/src/runtime_proxy/dispatch.rs` (~406; HTTP dispatch after websocket upgrade split)
- `crates/prodex-app/src/runtime_proxy/dispatch/websocket.rs` (~159; runtime websocket upgrade/capture handling)
- `crates/prodex-app/src/runtime_proxy/responses.rs` (~486; responses selection loop after quota/local-selection outcome splits)
- `crates/prodex-app/src/runtime_proxy/responses/quota_blocked.rs` (~116; responses quota-blocked retry/fallback handling)
- `crates/prodex-app/src/runtime_proxy/responses/local_selection.rs` (~95; responses local-selection blocked handling)
- `crates/prodex-app/src/runtime_proxy/affinity.rs` (~305; route affinity derivation after prompt-cache split)
- `crates/prodex-app/src/runtime_proxy/affinity/prompt_cache.rs` (~254; prompt-cache profile affinity state)
- `crates/prodex-app/src/runtime_proxy/response_forwarding.rs` (~391; response success forwarding after SSE tap reader split)
- `crates/prodex-app/src/runtime_proxy/response_forwarding/sse_tap.rs` (~155; SSE tap reader and stream side effects)
- `crates/prodex-app/src/runtime_proxy/presidio.rs` (~25; Presidio module facade)
- `crates/prodex-app/src/runtime_proxy/presidio/analyzer.rs` (~92; Presidio language detection and analyzer result merge helpers)
- `crates/prodex-app/src/runtime_proxy/presidio/engine.rs` (~278; shared typed inspection/redaction execution)
- `crates/prodex-app/src/runtime_proxy/presidio/findings.rs` (~181; bounded finding normalization and Unicode offset conversion)
- `crates/prodex-app/src/runtime_proxy/presidio/http.rs` (~223; HTTP request adaptation and result application)
- `crates/prodex-app/src/runtime_proxy/presidio/json_body.rs` (~281; JSON string extraction and replacement helpers)
- `crates/prodex-app/src/runtime_proxy/presidio/local.rs` (~629; local detector execution)
- `crates/prodex-app/src/runtime_proxy/presidio/registry.rs` (~90; bounded per-log-path runtime registry)
- `crates/prodex-app/src/runtime_proxy/presidio/telemetry.rs` (~225; bounded content-free metrics and structured logging)
- `crates/prodex-app/src/runtime_proxy/presidio/websocket.rs` (~250; WebSocket text adaptation)
- `crates/prodex-app/src/runtime_proxy/presidio/tests.rs` (~395; transport and detector failure-matrix characterization)
- `crates/prodex-app/src/runtime_proxy/lineage/remember.rs` (~438; non-compact lineage recording after compact lineage split)
- `crates/prodex-app/src/runtime_proxy/lineage/remember/compact.rs` (~136; compact-route session/turn lineage recording)
- `crates/prodex-app/src/runtime_proxy/quota/auto_redeem.rs` (~243; auto-redeem execution orchestration after pool and summary splits)
- `crates/prodex-app/src/runtime_proxy/quota/auto_redeem/pool.rs` (~263; auto-redeem pool probing and candidate selection)
- `crates/prodex-app/src/runtime_proxy/quota/auto_redeem/summary.rs` (~172; auto-redeem quota summary predicates and characterization tests)
- `crates/prodex-app/src/runtime_proxy/lifecycle.rs` (~384; proxy drop/admission/queue lifecycle after audit and wait splits)
- `crates/prodex-app/src/runtime_proxy/lifecycle/audit.rs` (~129; startup state audit and stale runtime-state pruning)
- `crates/prodex-app/src/runtime_proxy/lifecycle/wait.rs` (~122; inflight/probe wait outcome helpers)
- `crates/prodex-app/src/runtime_proxy/smart_context/cooldown.rs` (~31; smart-context temporary disable cooldown state)
- `crates/prodex-app/src/runtime_proxy/smart_context/rollout.rs` (~53; smart-context rollout and environment flag helpers)
- `crates/prodex-app/src/runtime_proxy/smart_context.rs` (~441; smart-context orchestration after cooldown, rollout, and static-observation splits)
- `crates/prodex-app/src/runtime_proxy/smart_context/static_observation.rs` (~100; static-context observation and artifact fingerprint updates)
- `crates/prodex-app/src/runtime_proxy/smart_context/static_context.rs` (~191; static-context cross-field/chunk dedupe after section split)
- `crates/prodex-app/src/runtime_proxy/smart_context/static_context/sections.rs` (~421; heading-section dedupe and persisted fingerprints)
- `crates/prodex-app/src/runtime_proxy/smart_context/rehydration.rs` (~416; surgical rehydration after appendix helper split)
- `crates/prodex-app/src/runtime_proxy/smart_context/rehydration/appendix.rs` (~187; exact appendix rendering and rehydrate-budget helpers)
- `crates/prodex-app/src/runtime_proxy/smart_context/rewrite_telemetry.rs` (~463; smart-context rewrite telemetry after label helper split)
- `crates/prodex-app/src/runtime_proxy/smart_context/rewrite_telemetry/labels.rs` (~147; telemetry label and category formatting helpers)
- `crates/prodex-runtime-proxy/src/gateway_policy.rs` (~274; public gateway-policy exports and characterization tests after routing/virtual-key splits)
- `crates/prodex-runtime-proxy/src/gateway_policy/routing.rs` (~253; route alias strategy and model rewrite decisions)
- `crates/prodex-runtime-proxy/src/gateway_policy/virtual_keys.rs` (~181; virtual-key auth, admission, limits, and usage bookkeeping)
- `crates/prodex-runtime-proxy/src/smart_context/replay.rs` (~148; replay public types, thresholds, JSON parsing, and module wiring after evaluator split)
- `crates/prodex-runtime-proxy/src/smart_context/replay/evaluation.rs` (~239; replay corpus evaluation and aggregate metric math after acceptance split)
- `crates/prodex-runtime-proxy/src/smart_context/replay/evaluation/acceptance.rs` (~272; replay acceptance failures, scenario failures, required coverage checks, and failure formatting)
- `crates/prodex-runtime-proxy/src/smart_context/replay/markdown.rs` (~188; replay evaluation Markdown rendering)

Priority split targets:

1. runtime/provider hot paths:
   - re-scan; remaining provider/runtime >500-line entries in this list are tests
2. gateway/config concentration:
   - re-scan after the OpenAPI path split; no remaining >500-line gateway OpenAPI path file
3. opt-in protocol broker:
   - `app_server_broker_protocol.rs` (~356)
   - `app_server_broker_preview.rs` (~310; stdio preview and contract after logging/report splits)
   - `app_server_broker_preview/logging.rs` (~332; runtime-log and audit emission for preview sessions)
   - `app_server_broker_preview/report.rs` (~212; preview report aggregation counts)

### 7. Existing tests that must be preserved

High-value current coverage already exists and should be treated as characterization:

- provider conformance fixtures:
  - `crates/prodex-provider-core/tests/provider_conformance.rs`
  - `crates/prodex-provider-core/tests/provider_conformance_v1.rs`
- compat replay fixtures:
  - `crates/prodex-app/tests/fixtures/compat_replay/*`
- runtime proxy coverage:
  - `crates/prodex-app/tests/src/runtime_proxy/*`
  - `crates/prodex-app/tests/support/main_internal/runtime_proxy_*`
- gateway admin CRUD/security coverage now rejects generated runtime virtual keys on admin endpoints
- app-server broker contract, validation, and live-pump tests:
  - `crates/prodex-app/tests/support/main_internal_body/app_server_broker.rs`
- runtime broker metrics / doctor / quota support tests

These are the current regression net. Do not delete heuristics that they indirectly cover before new fixtures exist.

### 8. Missing conformance tests

Missing or incomplete compared to the target state:

- Anthropic and Copilot now have tested Responses chat-compat coverage in provider-core, but their non-Responses endpoint depth still trails Gemini / DeepSeek
- Kiro conformance depth still trails Gemini / DeepSeek even though chat-completions degraded request/response mapping, invalid response-shape rejection, rejected control, legacy function mapping, and passthrough fixture coverage exists
- fixture-backed unsupported/degraded parameter matrix per provider/model for more endpoints
- provider stream termination / error fixture matrix across all supported providers
- app-server JSON-RPC schema fixtures and drift detection
- app-server thread / turn / item lifecycle replay tests
- gateway auth / budget / virtual-key / route-fallback integration matrix
- security regression matrix for malformed frames, oversized payloads, key leakage, role separation

### 9. Likely regression risks

Highest risk seams:

- Gemini and DeepSeek rewrites: behavior is still split between provider-core and runtime helpers
- Anthropic request forwarding now prefers provider-core in one live path, so drift between provider-core and runtime pending-message state is a new seam to watch
- SSE normalization and stream ordering
- continuation affinity if request classification changes too early
- gateway auth / virtual-key behavior if config parsing is moved without characterization tests
- spend / usage accounting if retries and fallback move behind new abstractions
- app-server broker work accidentally changing passthrough defaults

## Milestone plan

### P0 — canonical provider core and provider conformance

1. freeze behavior with more characterization tests around current app-side rewrites
2. finish moving pure Gemini / DeepSeek / Kiro transforms behind `prodex-provider-core`
3. replace app-side chat-compatible fallback helpers with provider-core-backed bridges wherever pending-message/session state is not required
4. deepen Anthropic and Copilot coverage beyond the current Responses chat-compat path
5. reduce `provider_bridge.rs` to orchestration + logging + compatibility shims
6. keep `docs/provider-conformance.md` and `docs/provider-capabilities.md` synced with the actual contract matrix

### P1 — protocol-aware app-server broker and gateway/control-plane maturation

1. preserve the current read-only diagnostic stdio mode and passthrough-aware preview surfaces
2. extend the current broker diagnostics with schema-aware and replay-backed validation:
   - keep JSON-RPC envelope parsing
   - keep lifecycle metadata extraction
   - keep lifecycle schema-file hints tied to imported upstream Codex schema fixtures
   - keep fixture-backed diagnostic/request/response summaries
   - keep live passthrough observation and replay fixtures
3. add schema-aware drift tests
4. split gateway launch/config into:
   - auth
   - virtual keys
   - state store
   - route aliases
   - guardrails
   - observability
   - provider/upstream selection
   - SSO/OIDC
5. keep passthrough default until diagnostics and replay tests are solid

### Current Phase 2A broker status

Already implemented in `crates/prodex-app/src/app_server_broker.rs` with fixture and helper coverage in
`crates/prodex-app/tests/support/main_internal_body/app_server_broker.rs`:

- experimental line-by-line `stdio-preview` mode behind `app-server-broker --experimental-stdio`
- experimental passthrough-aware read-only mode behind
  `app-server-broker --experimental-stdio-passthrough-preview`
- experimental fail-closed validation mode behind
  `app-server-broker --experimental-stdio-validate`
- experimental validate-before-forward passthrough mode behind
  `app-server-broker --experimental-stdio-validate-passthrough`
- validation modes now perform minimal lifecycle-order checks for turn notifications:
  - JSON-RPC response frames must match an observed pending request id
  - duplicate pending request ids are rejected until the original response closes the pending entry
  - pending request ids left open at EOF are rejected
  - successful lifecycle responses must return the `thread.id`, valid thread response context, valid returned thread object context, `turn.id`, `turn.items`, and valid thread/turn status needed for affinity tracking/schema checks, including valid `activeFlags` when a thread is `active`
  - lifecycle frames that require thread affinity reject missing `thread_id` / `threadId`
  - `thread/started` notifications must carry the required `params.thread.id`, required thread object context, and valid `thread.status.type`, including valid `activeFlags` when the thread is `active`
  - `turn/start` requests must carry the required `input` array
  - turn notifications must carry `turn.items` and a valid `turn.status`
  - `turn/started` must carry a turn id
  - `turn/completed` must carry a turn id
  - `turn/completed` cannot appear before `turn/started` for the same turn
  - a thread cannot receive a second active `turn/started` before the first turn completes
  - `turn/interrupt` must carry thread/turn ids and cannot target a different active turn
  - duplicate `turn/started` / `turn/completed` events for the same turn are rejected
- command-layer seams for both broker surfaces in `crates/prodex-app/src/app_commands/app_server_broker.rs`:
  - `render_app_server_broker_output(json)`
  - `run_app_server_broker_stdio_preview(reader, writer)`
  - `run_app_server_broker_stdio_passthrough_preview(reader, passthrough_writer, diagnostics_writer)`
  - `run_app_server_broker_stdio_validate(reader, writer)`
  - `run_app_server_broker_stdio_validate_passthrough(reader, passthrough_writer, diagnostics_writer)`
- broker-core read-only passthrough helper in `crates/prodex-app/src/app_server_broker.rs`:
  - `app_server_broker_write_stdio_passthrough_preview_stream(reader, passthrough_writer, diagnostics_writer)`
  - `app_server_broker_write_stdio_validate_stream(reader, writer)`
  - `app_server_broker_write_stdio_validate_passthrough_stream(reader, passthrough_writer, diagnostics_writer)`
- frame-kind classification for `request` / `notification` / `response` / `invalid`
- upstream wire-format compatibility for omitted `jsonrpc` headers on JSONL app-server messages
- lifecycle detection with alias/spacing normalization
- request parsing with raw-method preservation
- diagnostic summary JSON carrying:
  - `valid_jsonrpc`
  - `frame_kind`
  - `id`
  - `method`
  - `method_kind`
  - `is_lifecycle_method`
  - `invalid_reason`
  - metadata IDs
- preview-line oversized-frame guard:
  - lines over 1 MiB are reported as `line_too_large` before JSON parsing
  - stdio reads are bounded so an oversized no-newline frame fails closed instead of allocating unbounded memory
- response summary JSON carrying:
  - `id`
  - `has_result`
  - `has_error`
  - `error_code`
  - `error_message`
- invalid-reason taxonomy currently implemented:
  - `non_jsonrpc_version`
  - `batch_frame_unsupported`
  - `non_object_frame`
  - `non_scalar_id`
  - `non_container_params`
  - `non_object_error`
  - `non_integer_error_code`
  - `non_string_error_message`
  - `non_string_method`
  - `invalid_method_name`
  - `result_with_error`
  - `missing_response_id`
  - `method_with_result_or_error`
  - `missing_method_and_response_payload`
- stdin preview/report surfaces for newline-delimited JSON input:
  - passthrough-aware read-only mode can mirror raw input unchanged while emitting the same diagnostics on a side channel
  - validate-before-forward mode mirrors only JSON/protocol-valid input and fails before forwarding malformed frames
  - validation mode also rejects known-invalid turn notification ordering before policy routing is introduced
  - JSONL preview events per nonblank input line
  - preserved input line numbers in each preview event
  - wrapped pretty JSON preview-report envelope for one-shot rendering:
    - `{"object":"app_server_broker.preview_report","report":...}`
  - final wrapped session summary object at EOF with the same envelope shape
  - aggregate preview report with:
    - parse/error counts
    - frame-kind counts
    - method-kind counts
    - invalid-reason counts
  - local audit summary event per preview session:
    - component `app_server_broker`
    - action `preview_session`
    - outcome `observed`
    - counts-only payload for mode/frame/continuation policy/routing summary
    - provider-switch allowed / requires-override counts
- fixture-backed corpora and golden replay coverage for:
  - diagnostics
  - request parsing/summary
  - frame classification/response summary
  - valid stdio preview transcripts
  - malformed stdio preview transcripts
- helper matrix tests for:
  - lifecycle detection
  - method-kind classification
  - invalid-reason classification
- command/helper path coverage for:
  - human render output
  - JSON contract output
  - stdio preview replay output
  - stdio passthrough replay output
  - malformed stdio preview output
  - malformed stdio passthrough replay output
  - empty stdin session summary
  - empty stdin passthrough summary
  - contextual write-failure propagation
  - contextual passthrough write-failure propagation

Phase 2A is now covered at the current diagnostic scope:

- passthrough-aware read-only stdio observation exists
- parsed request/session/turn metadata is written to the runtime log during preview/passthrough observation
- preview diagnostic summaries redact secret-like JSON-RPC ids, methods, metadata, and affinity values before rendering
- runtime log `frame_id`, `request_id`, method, and metadata values reuse shared secret-like redaction before rendering
- response-summary diagnostics redact secret-like JSON-RPC error messages before rendering
- tests prove live app-server traffic can be observed without changing passthrough routing behavior
- tests prove the opt-in validate-before-forward path stops malformed JSON/protocol frames before stdout passthrough
- tests prove validation catches response-without-request, duplicate pending request ids, pending requests left open at EOF, lifecycle responses missing affinity ids or valid thread/turn status, missing thread-affinity payloads, malformed `thread/started` payloads, missing `turn/start` input, missing or invalid turn notification status, turn-completed-before-started ordering, same-thread active-turn conflicts, and wrong-turn interrupt conflicts in both diagnostics-only and validate-before-forward paths

Phase 2B bootstrap now exists at the fixture/drift level:

- checked-in protocol surface manifest:
  - `crates/prodex-app/tests/fixtures/compat_replay/app_server_broker_protocol_surface.json`
- public broker contract now exposes the local validation state:
  - `schema_validation.protocol_surface_fixture = true`
  - the checked-in protocol surface fixture now also declares the broker affinity policy taxonomy
  - `schema_validation.fixture_drift_tests = true`
  - `schema_validation.helper_consistency_tests = true`
  - `schema_validation.stream_helper_consistency_tests = true`
  - `schema_validation.cross_surface_consistency_tests = true`
  - `schema_validation.report_aggregation_consistency_tests = true`
  - `accepted_lifecycle_aliases = ["notifications/initialized", "turn/cancel"]`
  - `affinity.decision_kinds = ["fresh", "continue-session", "continue-thread", "continue-turn"]`
  - `schema_validation.metadata_surface_fixture = true`
  - `schema_validation.metadata_drift_tests = true`
  - `schema_validation.output_surface_fixture = true`
  - `schema_validation.output_drift_tests = true`
  - `schema_validation.runtime_log_surface_fixture = true`
  - `schema_validation.runtime_log_drift_tests = true`
  - `schema_validation.upstream_codex_schema_imported = true`
  - `schema_validation.lifecycle_response_schema_hints = true`
- checked-in imported upstream Codex generated schema artifacts now cover the first lifecycle payloads:
  - `crates/prodex-app/tests/fixtures/compat_replay/upstream_codex_schema/ThreadStartParams.json`
  - `crates/prodex-app/tests/fixtures/compat_replay/upstream_codex_schema/TurnStartParams.json`
- schema-valid replay corpus now binds lifecycle request/result/notification bodies to imported upstream schema subsets:
  - required fields
  - nested object shape
  - basic type checks
  - enum checks
  - array item checks
  - `anyOf` / `oneOf` / `allOf` reference selection used by the imported lifecycle schemas
  - `crates/prodex-app/tests/fixtures/compat_replay/app_server_broker_upstream_schema_cases.json`
- multi-line preview and stdio preview streams now correlate JSON-RPC response ids back to lifecycle request ids so response diagnostics can report `ThreadStartResponse`, `ThreadResumeResponse`, `ThreadForkResponse`, `TurnStartResponse`, and `TurnInterruptResponse` schema hints without changing passthrough routing
- synthetic JSONL replay now exercises the broker preview/report/stream helpers with that same upstream-backed lifecycle corpus:
  - `crates/prodex-app/tests/fixtures/compat_replay/app_server_broker_upstream_schema_replay.txt`
  - `crates/prodex-app/tests/fixtures/compat_replay/app_server_broker_upstream_schema_expected_report.json`
  - `crates/prodex-app/tests/fixtures/compat_replay/app_server_broker_upstream_schema_expected_stream.jsonl`
- passthrough-preview now also has explicit upstream-backed stdout/stderr drift fixtures:
  - `crates/prodex-app/tests/fixtures/compat_replay/app_server_broker_upstream_schema_passthrough_expected_stdout.txt`
  - `crates/prodex-app/tests/fixtures/compat_replay/app_server_broker_upstream_schema_passthrough_expected_stderr.jsonl`
- normalized runtime-log fixture now covers parsed observe/summary fields for the same upstream replay:
  - `crates/prodex-app/tests/fixtures/compat_replay/app_server_broker_upstream_schema_expected_runtime_log.json`
- broker core now has an explicit lifecycle-stage classifier for request/notification shapes already present in the imported upstream replay corpus:
  - initialize request
  - initialized notification
  - thread start / started / resume / fork
  - turn start / started / completed / interrupt
  - runtime log observe events now include this normalized `lifecycle_stage` field
  - preview JSON diagnostic summaries now also expose `lifecycle_stage`
- broker core now also has a normalized lifecycle binding helper that pairs:
  - lifecycle stage
  - session id
  - thread id
  - turn id
  - this is the first explicit continuation-affinity prep hook built on top of the imported upstream replay corpus
- broker core now also derives ordered affinity keys from those bindings:
  - turn → thread → session for turn lifecycle
  - thread → session for thread lifecycle
  - session-only for initialization lifecycle
  - preview JSON diagnostic summaries now also expose these ordered `affinity_keys`
- broker core now has a provider-switch policy helper:
  - fresh/open frames may switch provider/account
  - session/thread/turn continuations require an explicit override
  - runtime log observe events expose `provider_switch_allowed` and `provider_switch_requires_override`
- diagnostic summaries now also expose `primary_affinity` as the first/strongest ordered affinity key
- diagnostic summaries now also expose `continuation_affinity` as a broker-owned ownership summary:
  - lifecycle stage
  - owner kind: none / session / thread / turn
  - owner
  - primary affinity
  - affinity key count
  - turn/thread/session presence flags
- diagnostic summaries now also expose `continuation_decision` as a minimal policy stub:
  - `fresh`
  - `continue-session`
  - `continue-thread`
  - `continue-turn`
- runtime log observe events now also expose `continuation_decision`
- the broker lifecycle taxonomy now follows the upstream interrupt method name `turn/interrupt` while still accepting the older local `turn/cancel` alias in diagnostics for compatibility
- drift tests now bind helper taxonomy, fixture corpora, and replay transcripts to the same declared lifecycle/frame/method/invalid-reason surface
- helper consistency tests now bind:
  - `preview_line.summary` ↔ `diagnostic_summary_json`
  - `request_summary_json` ↔ `diagnostic_summary_json` overlap
  - `response_summary_json` ↔ `diagnostic_summary_json` overlap
- stream/helper consistency tests now bind:
  - `preview_lines(input)` ↔ streamed preview-event sequence
  - `preview_report(input)` ↔ streamed EOF summary report
- preview diagnostic summaries and runtime log observe events redact secret-looking JSON-RPC string fields before rendering derived diagnostic fields
- response-summary diagnostics redact secret-looking JSON-RPC error messages before rendering
- metadata-surface drift tests now bind:
  - direct snake_case fields
  - direct camelCase fields
  - `client_metadata`
  - nested `thread.id` / `turn.id` / `item.id`
  - `context`
  - precedence, trimming, and non-JSON-RPC ignore behavior
- output-surface drift tests now bind:
  - raw preview-report fixture bodies
  - JSONL preview-event fixtures
  - wrapped EOF preview-report envelopes
- runtime-log drift tests now bind:
  - `app_server_broker_observe`
  - `app_server_broker_summary`
  - required parsed-frame / parse-error / summary field sets
- cross-surface consistency tests now bind:
  - diagnostics EOF summary counts
  - runtime-log summary counts
  - observe-line count vs diagnostics preview count
- report aggregation consistency tests now bind:
  - `line_count`
  - `parsed_count`
  - `error_count`
  - frame/method/invalid-reason counts
  - all derived directly from attached `previews`
- this is still a local compatibility harness, not yet imported upstream Codex schema material

### P2 — observability, security hardening, hot-path refactor, broader tests

1. unify correlation fields across proxy, gateway, and future broker
2. add explicit redaction checks and role separation tests
3. split remaining large runtime/provider files by responsibility
4. expand fixture and integration matrix

### P3 — docs polish and developer ergonomics

1. architecture overview
2. provider-addition guide
3. gateway production assumptions
4. broker modes and migration notes

## Smallest safe first implementation

The smallest high-value next patch is:

1. add characterization tests for app-side Gemini / DeepSeek bridge behavior that still matters
2. extract one more pure rewrite slice from app runtime into `prodex-provider-core`
3. keep existing runtime entrypoints as wrappers

Recommended first extraction target:

- Gemini request-shape admission / conformance path, because:
  - provider core already has a Gemini translator
  - app side still carries large Gemini-specific rewrite logic
  - `provider_bridge.rs` is already the intended seam

## Stop rules

Do not do these in the same patch as the first extraction:

- live app-server broker mode
- gateway auth redesign
- retry / affinity behavior changes
- transport rewrites

Those need separate characterization nets.
