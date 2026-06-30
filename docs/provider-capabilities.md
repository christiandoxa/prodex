# Provider Capabilities

Generated from `crates/prodex-provider-core/tests/fixtures/provider_contracts.json`, which is checked against `ProviderAdapterContract`, and `scripts/catalog/provider-catalog-check.mjs`.

| Provider | Models | Transform | Streaming | Fallback | responses | chat-completions | messages | embeddings | images | audio | batches | rerank | a2a |
|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|
| openai | 4 | passthrough | true | false | native | native | native | native | native | native | native | native | native |
| anthropic | 9 | translated | true | true | translated | translated | translated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| copilot | 25 | translated | true | true | translated | translated | translated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| deepseek | 4 | translated | true | true | translated | translated | translated | unsupported | unsupported | unsupported | unsupported | unsupported | unsupported |
| gemini | 19 | translated | true | true | translated | translated | translated | translated | unsupported | unsupported | unsupported | unsupported | unsupported |
| local | 1 | passthrough | true | false | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough | passthrough |

Status values: `native`, `translated`, `passthrough`, `unsupported`.
