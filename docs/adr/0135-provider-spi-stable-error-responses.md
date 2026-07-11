# ADR 0135: Provider SPI stable error responses

## Status

Accepted

## Context

`prodex-provider-spi` validates provider routes and invocation envelopes before
adapter-specific transport work begins. Raw validation errors can reveal model
lengths, credential scope internals, tenant identifiers, or provider credential
references. Those details are useful for trusted diagnostics but should not be
returned through client-visible gateway responses.

## Decision

Add `plan_provider_route_error_response` and
`plan_provider_invocation_error_response` to `prodex-provider-spi`. Route
validation failures map to `provider_route_invalid`; invocation validation
failures map to `provider_invocation_not_authorized`.

The response plans are HTTP-neutral and expose only status, code, and generic
message fields. Raw errors remain available for trusted redacted diagnostics.

## Consequences

Gateway composition roots can reject malformed provider routing and unauthorized
provider invocation envelopes consistently without leaking model names or
lengths, tenant IDs, credential scope details, or provider secret references.
