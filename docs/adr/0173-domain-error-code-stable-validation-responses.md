# 0173: Domain Error Code Stable Validation Responses

## Status

Accepted

## Context

Enterprise APIs expose machine-readable error envelopes. The envelope code must
remain stable, parseable, and safe to index across gateway and control-plane
clients. Raw adapter or provider failures can otherwise produce uppercase text,
path fragments, whitespace, copied credentials, or overlong high-cardinality
values as error codes.

The compatibility constructor remains available for existing call sites, but new
API boundaries need a validated path and a redacted response plan for rejected
codes.

## Decision

`prodex-domain` owns `ErrorCode::try_new`, `ErrorCode::validate`, and
`plan_error_code_error_response`.

Validated error codes are non-empty, at most 128 characters, composed from
lowercase ASCII letters, digits, underscores, and dot-separated non-empty
segments. Validation failures map to stable status/code/message response plans
that do not echo the rejected code, length, path fragment, tenant identifier,
request identifier, audit identifier, or credential-like material.

## Consequences

- Gateway and control-plane adapters can reject unstable error-code material
  before serializing public envelopes.
- Existing compatibility paths can keep using `ErrorCode::new` while migrating
  incrementally to the validated constructor.
- Raw invalid codes remain available only to trusted diagnostics.
