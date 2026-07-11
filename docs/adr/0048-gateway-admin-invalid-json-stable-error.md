# ADR 0048: Gateway admin invalid JSON errors use stable messages

## Status

Accepted

## Context

Gateway admin endpoints parse JSON request bodies for mutating control-plane-style operations such as virtual-key and SCIM management. The shared JSON parsing helper returned `invalid_json` with the raw `serde_json` parser error as the client-facing message.

Raw parser errors can expose implementation-specific details such as line/column positions, EOF wording, or parser internals. The enterprise API-governance target requires stable machine-readable error envelopes and non-leaking error responses.

## Decision

Gateway admin JSON parsing failures still return HTTP `400` with error code `invalid_json`, but the message is now stable:

```text
request body is not valid JSON
```

The parser error is not included in the client response.

## Consequences

Admin API clients retain a stable error code and status while avoiding dependency on parser-specific text. This also prevents accidental leakage of low-level parser diagnostics into control-plane API responses.

## Validation

A regression test sends malformed JSON to an authenticated admin key-create endpoint and verifies that:

- status is `400`;
- error code is `invalid_json`;
- message is the stable generic text;
- parser details such as line/column/EOF and the admin token are absent.
