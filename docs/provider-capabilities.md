# Provider Capabilities

Generated from `prodex_provider_core::provider_contract_catalog()`, `crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json`, and `crates/prodex-provider-core/catalog/models.json`.

| Provider | Models | Transform | Streaming | Fallback | Fixtures req/resp/stream | responses | responses/compact | chat-completions | messages | models | embeddings | images | audio | batches | rerank | a2a |
|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| openai | 6 | passthrough | true | false | 7/7/1 | native | emulated | native | unsupported | native | native | native | native | native | unsupported | unsupported |
| anthropic | 9 | translated | true | true | 12/5/4 | translated | emulated | passthrough | passthrough | emulated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| copilot | 26 | passthrough | true | true | 10/5/2 | native | passthrough | passthrough | passthrough | emulated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| deepseek | 4 | translated | true | true | 16/4/3 | translated | emulated | passthrough | passthrough | emulated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| gemini | 19 | translated | true | true | 18/14/3 | translated | emulated | passthrough | passthrough | emulated | passthrough | unsupported | unsupported | unsupported | unsupported | unsupported |
| kiro | 1 | translated | true | false | 9/6/1 | translated | emulated | translated | translated | emulated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| local | 1 | passthrough | true | false | 10/10/1 | passthrough | emulated | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough |

Status values: `native`, `translated`, `passthrough`, `emulated`, `partial`, `untested`, `unsupported`.

Fixture summary counts are `request/response/stream-event` conformance cases per provider.

Model counts cover deterministic offline built-ins. Imported or provider-discovered runtime routes may augment them, and Super accepts an explicit non-empty custom child model ID without requiring live discovery.

## Harness modes

Default mode: `native`. Resolved mode for this catalog: `native`.

| Mode | Label | Selectable | Default effective | Canonical request routes | Request shaping | Response shaping | Stream shaping | Description |
|---|---|---|---|---|---|---|---|---|
| native | Native | true | native | responses, responses/compact, chat-completions, messages, models, embeddings, images, audio, batches, rerank, a2a | false | false | false | Preserves existing bridge behavior without harness shaping. |
| minimal | Minimal | true | minimal | responses | true | false | false | Prepends the minimal/v1 instruction block to canonical Responses requests. |
| evaluated | Evaluated | true | evaluated | responses | true | true | true | Applies only provider/model policies backed by the versioned evaluation catalog. |

## Declared Responses parameter limitations

- `anthropic`: `input[*].content[type!=text]`, `response_format.type`, `reasoning`, `text.format`, `n>1`, `metadata`, `safety_identifier`, `web_search_options`, `input[type=custom_tool_call|tool_search_call]`, `messages`, `tools[type!=function]`, `tool_choice[type!=function]`, `parallel_tool_calls=false`, `logprobs/top_logprobs`, `stop_sequences`, `previous_response_id`
- `deepseek`: `parallel_tool_calls=false`, `web_search_options`, `safety_identifier`, `tools[type!=function]`, `input[*].content[type!=text]`
- `gemini`: `response_format.type`
- `kiro`: `temperature/top_p`, `stop/stop_sequences`, `logprobs/top_logprobs`, `response_format/text.format[type!=text]`, `tool_choice!=auto`, `tools/web_search_options`, `parallel_tool_calls=false`, `input[*].content[type!=text]`, `max_output_tokens/max_tokens/max_completion_tokens`

## Semantic compact observability

Gemini and Kiro semantic compact responses expose `x-prodex-compact-mode` (`semantic` or `local-fallback`) and `x-prodex-compact-provider`. Lossy fallback also exposes `x-prodex-compact-degraded: true` plus a bounded `x-prodex-compact-reason` code: `timeout`, `unsupported`, `unavailable`, `invalid-response`, `provider-error`, or `local-policy`. Raw upstream errors are never copied into headers.

Prometheus output includes `prodex_semantic_compact_total{provider,mode}` and `prodex_semantic_compact_fallback_total{provider,reason}` with fixed-cardinality labels. Local fallback preserves HTTP 200 for continuation compatibility but is not semantic success. It is intentionally lossy and retains at most 24 recent snippets, 768 bytes per snippet, and 24 KiB total.

## Transport limits

Capability labels describe documented HTTP/text transformations, not lossless equivalence. Translated or emulated shapes may reject unsupported fields as listed above. Gemini Live rejects unexpected upstream binary WebSocket frames predictably; it does not reinterpret them as text.
