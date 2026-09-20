# Provider Capabilities

Generated from the current prodex_provider_core implementation registry and translator metadata, crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json, and crates/prodex-provider-core/catalog/models.json.

| Provider | Models | Transform | Streaming | Fallback | Fixtures req/resp/stream | responses | responses/compact | chat-completions | messages | models | embeddings | images | audio | batches | rerank | a2a |
|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| openai | 7 | passthrough | true | false | 7/7/1 | native | emulated | native | unsupported | native | native | native | native | native | unsupported | unsupported |
| anthropic | 9 | translated | true | true | 12/5/4 | translated | emulated | passthrough | passthrough | emulated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| copilot | 26 | passthrough | true | true | 10/5/2 | native | passthrough | passthrough | passthrough | emulated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| deepseek | 4 | translated | true | true | 16/4/3 | translated | emulated | passthrough | passthrough | emulated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| gemini | 19 | translated | true | true | 18/14/3 | translated | emulated | passthrough | passthrough | emulated | passthrough | unsupported | unsupported | unsupported | unsupported | unsupported |
| kiro | 2 | translated | true | false | 9/6/1 | translated | emulated | translated | translated | emulated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| local | 1 | passthrough | true | false | 10/10/1 | passthrough | emulated | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough |

Status values: `native`, `translated`, `passthrough`, `emulated`, `partial`, `untested`, `unsupported`.

Fixture summary counts are `request/response/stream-event` conformance cases per provider.

Model counts cover deterministic offline built-ins. Imported or provider-discovered runtime routes may augment them, and Super accepts an explicit non-empty custom child model ID without requiring live discovery.

## Declared Responses parameter limitations

- `anthropic`: `input[*].content[type!=text]`, `input[type=custom_tool_call|tool_search_call]`, `logprobs/top_logprobs`, `messages`, `metadata`, `n>1`, `parallel_tool_calls=false`, `previous_response_id`, `reasoning`, `response_format.type`, `safety_identifier`, `stop_sequences`, `text.format`, `tool_choice[type!=function]`, `tools[type!=function]`, `web_search_options`
- `deepseek`: `input[*].content[type!=text]`, `parallel_tool_calls=false`, `safety_identifier`, `tools[type!=function]`, `web_search_options`
- `gemini`: `response_format.type`
- `kiro`: `input[*].content[type!=text]`, `logprobs/top_logprobs`, `max_output_tokens/max_tokens/max_completion_tokens`, `parallel_tool_calls=false`, `response_format/text.format[type!=text]`, `stop/stop_sequences`, `temperature/top_p`, `tool_choice!=auto`, `tools/web_search_options`

## Semantic compact observability

Gemini and Kiro semantic compact responses expose `x-prodex-compact-mode` (`semantic` or `local-fallback`) and `x-prodex-compact-provider`. Lossy fallback also exposes `x-prodex-compact-degraded: true` plus a bounded `x-prodex-compact-reason` code: `timeout`, `unsupported`, `unavailable`, `invalid-response`, `provider-error`, or `local-policy`. Raw upstream errors are never copied into headers.

Prometheus output includes `prodex_semantic_compact_total{provider,mode}` and `prodex_semantic_compact_fallback_total{provider,reason}` with fixed-cardinality labels. Local fallback preserves HTTP 200 for continuation compatibility but is not semantic success. It is intentionally lossy and retains at most 24 recent snippets, 768 bytes per snippet, and 24 KiB total.

## Transport limits

Capability labels describe documented HTTP/text transformations, not lossless equivalence. Translated or emulated shapes may reject unsupported fields as listed above. Gemini Live rejects unexpected upstream binary WebSocket frames predictably; it does not reinterpret them as text.
