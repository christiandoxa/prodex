# Provider Conformance

`prodex-provider-core` is the source of truth for provider capability metadata,
model catalogs, parameter support, pure request/response/stream translation,
usage extraction, and provider error classification.

`prodex-app` owns runtime side effects:

- network transport and websocket lifecycles
- OAuth/API-key pools and credential refresh
- profile selection, fallback, retry, and quota policy
- continuation and session affinity
- request admission and commit timing

Keeping that boundary narrow makes provider behavior testable without moving
transport or rotation policy into translators.

## Capability negotiation

Provider selection and request admission use this order:

1. choose the provider
2. verify the endpoint through `provider_adapter(provider).supported_endpoints()`
3. inspect endpoint status through `provider_adapter_contract_matrix()` or
   `capability_status(endpoint)`
4. inspect model-aware parameter support through
   `provider_translator(provider).supported_params(endpoint, model)`
5. run `transform_request`, `transform_response`, or `transform_stream_event`

Adapter metadata answers whether an endpoint can be considered. Translator
support describes parameter-level loss. Transform results record what happened
to the concrete payload.

Catalog limits are optional. Missing context, output, reasoning, pricing, or
embedding compatibility data remains unknown and follows the configured
unknown-limit policy; Prodex does not invent zero values or runtime guesses.
Embedding fallback remains disabled unless vector space, dimensions, and
normalization compatibility are explicit.

## Loss states

Every transform reports one of:

- `lossless`
- `degraded`
- `rejected`
- `unsupported`

Every non-lossless result carries a reason. Runtime diagnostics expose that
reason, while retry, fallback, and profile rotation remain runtime decisions.
Silent parameter dropping is not a valid conformance result.

## Conformance evidence

The generated matrix in [provider-capabilities.md](provider-capabilities.md)
records endpoint status and request/response/stream fixture counts. A provider
with upstream limitations must include an explicit non-lossless fixture.

Minimum coverage for a provider is:

- request transformation
- buffered response transformation
- stream-event transformation and termination
- usage extraction
- structured error classification where the upstream exposes error codes
- at least one degraded, rejected, or unsupported case when limits exist

Run the provider checks with:

```bash
npm run catalog:providers
npm run docs:provider-capabilities:check
cargo test -q -p prodex-provider-core
```

## Adding a provider

1. add the `ProviderId` and catalog rows
2. implement `ProviderTranslator` under `crates/prodex-provider-core/src/translators/`
3. register adapter metadata and capability status
4. add request, response, stream, usage, and error fixtures
5. keep any app-side compatibility shim thin and side-effect-only
6. regenerate the capability matrix after fixtures pass

Pure predicates and payload shaping belong in `prodex-provider-core`; auth,
state, transport, and retry timing belong in `prodex-app`.

## App-server broker

`prodex app-server-broker --experimental-stdio-live` launches `codex app-server`,
validates lifecycle frames bidirectionally with one shared lifecycle state, and
preserves stdio passthrough. Default app-server traffic remains passthrough;
model HTTP traffic uses the normal silent runtime-proxy preparation.

The broker validates protocol drift but is not a second provider router. It
does not own provider selection or weaken `previous_response_id`, turn-state,
or `session_id` affinity.
