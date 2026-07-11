# ADR 0053: Redact provider response transformation failures

## Status

Accepted

## Context

The local gateway rewrites provider-specific buffered responses into Codex-compatible
responses for providers such as Gemini, DeepSeek, and Copilot-compatible upstreams.
When a provider response cannot be transformed, the fallback `502` response used
to include the raw transformation error. Those errors can contain parser details,
provider-specific payload shape, or other implementation diagnostics that are not
part of the public gateway contract.

## Decision

Buffered provider response transformation failures now return a stable redacted
message:

- status: `502`
- message: `provider response could not be transformed`

The raw transformation error is not used in the client-visible body. Successful
responses, streaming responses, provider pass-through behavior, and spend/guardrail
accounting are unchanged. Copilot-compatible buffered Responses rewrites use the
same redacted fallback as the other buffered provider rewrite paths.

## Consequences

- Clients receive deterministic failure text for provider response conversion
  errors.
- Parser/provider internals are kept out of gateway responses in line with the
  secure-by-default API error policy.
- Unit coverage pins the stable redacted message and rejects common internal
  provider/parser detail strings.
