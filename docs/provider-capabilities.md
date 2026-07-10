# Provider Capabilities

Generated from `prodex_provider_core::provider_adapter_contract_matrix()`, `crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json`, and `crates/prodex-provider-core/catalog/models.json`.

| Provider | Models | Transform | Streaming | Fallback | Fixtures req/resp/stream | responses | responses/compact | chat-completions | messages | models | embeddings | images | audio | batches | rerank | a2a |
|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| openai | 5 | passthrough | true | false | 10/10/1 | native | unsupported | native | native | native | native | native | native | native | native | native |
| anthropic | 9 | translated | true | true | 12/5/4 | translated | unsupported | passthrough | passthrough | untested | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| copilot | 25 | translated | true | true | 10/6/2 | translated | passthrough | passthrough | passthrough | untested | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| deepseek | 4 | translated | true | true | 16/4/3 | translated | unsupported | passthrough | passthrough | untested | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| gemini | 19 | translated | true | true | 18/14/3 | translated | unsupported | passthrough | passthrough | untested | passthrough | unsupported | unsupported | unsupported | unsupported | unsupported |
| kiro | 2 | translated | true | false | 9/6/1 | translated | emulated | translated | translated | emulated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| local | 1 | passthrough | true | false | 10/10/1 | passthrough | unsupported | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough |

Status values: `native`, `translated`, `passthrough`, `emulated`, `partial`, `untested`, `unsupported`.

Fixture summary counts are `request/response/stream-event` conformance cases per provider.

## Declared Responses parameter limitations

- `anthropic`: `input[*].content[type!=text]`, `response_format.type`, `reasoning`, `text.format`, `n>1`, `metadata`, `safety_identifier`, `web_search_options`, `input[type=custom_tool_call|tool_search_call]`, `messages`, `tools[type!=function]`, `tool_choice[type!=function]`, `parallel_tool_calls=false`, `logprobs/top_logprobs`, `stop_sequences`, `previous_response_id`
- `copilot`: `input[*].content[type!=text]`, `response_format.type`, `reasoning`, `text.format`, `n>1`, `metadata`, `safety_identifier`, `web_search_options`, `input[type=custom_tool_call|tool_search_call]`, `messages`, `tools[type!=function]`, `tool_choice[type!=function]`, `parallel_tool_calls=false`, `logprobs/top_logprobs`, `stop_sequences`, `previous_response_id`
- `deepseek`: `parallel_tool_calls=false`, `web_search_options`, `safety_identifier`, `tools[type!=function]`, `input[*].content[type!=text]`
- `gemini`: `response_format.type`
