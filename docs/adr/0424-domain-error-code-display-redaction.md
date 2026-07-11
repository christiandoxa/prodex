# 0424: Domain error code display redaction

## Status

Accepted

## Context

Error codes are machine-readable boundary inputs for API envelopes. Response
planning already redacts invalid values, but generic error display text can be
logged before response planning.

## Decision

`ErrorCodeError::Display` no longer includes the rejected error-code length.

## Consequences

- Invalid error-code metadata stays out of generic error plumbing.
- Error response codes and messages remain unchanged.
