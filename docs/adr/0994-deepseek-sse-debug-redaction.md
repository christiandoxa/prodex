# ADR 0994: DeepSeek SSE debug redaction

## Status

Accepted.

## Context

DeepSeek SSE state accumulates response IDs, streamed output text, reasoning
content, refusals, tool-call names and arguments, usage, logprobs,
annotations, fingerprints, response metadata, and conversation messages.
Derived `Debug` output can leak prompt, response, reasoning, or tool payloads
through panic output, assertions, or diagnostics.

## Decision

`RuntimeDeepSeekSseState` and `RuntimeDeepSeekToolCall` use hand-written
`Debug` implementations that preserve state flags, completion shape, and
collection presence while redacting response identifiers, model names, streamed
content, tool payloads, metadata, usage, logprobs, fingerprints, and
conversation messages.

## Consequences

DeepSeek SSE translation behavior is unchanged. Diagnostics can still identify
stream state shape without exposing prompt, model, tool, or streamed response
content. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse_tests.rs`.
