# Provider Capabilities

Generated from `prodex_provider_core::provider_adapter_contract_matrix()`, `crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json`, and `crates/prodex-provider-core/catalog/models.json`.

| Provider | Models | Transform | Streaming | Fallback | Fixtures req/resp/stream | responses | chat-completions | messages | models | embeddings | images | audio | batches | rerank | a2a |
|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| openai | 4 | passthrough | true | false | 10/10/1 | native | native | native | native | native | native | native | native | native | native |
| anthropic | 9 | translated | true | true | 3/3/1 | translated | passthrough | passthrough | untested | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| copilot | 25 | translated | true | true | 3/3/1 | translated | passthrough | passthrough | untested | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| deepseek | 4 | translated | true | true | 14/4/2 | translated | passthrough | passthrough | untested | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| gemini | 19 | translated | true | true | 16/14/3 | translated | passthrough | passthrough | untested | passthrough | unsupported | unsupported | unsupported | unsupported | unsupported |
| local | 1 | passthrough | true | false | 10/10/1 | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough |

Status values: `native`, `translated`, `passthrough`, `unsupported`, `partial`, `untested`.

Fixture summary counts are `request/response/stream-event` conformance cases per provider.
