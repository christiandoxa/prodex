# ADR 0993: DeepSeek request debug redaction

## Status

Accepted.

## Context

DeepSeek pending and translated request DTOs carry prompt messages, translated
request bodies, and response metadata. Derived `Debug` output can leak prompt or
metadata payloads through panic output, assertions, or runtime diagnostics.

## Decision

`RuntimeDeepSeekPendingRequest` and `RuntimeDeepSeekTranslatedRequest` use
hand-written `Debug` implementations that preserve DTO shape and collection
presence while redacting message, body, and metadata payloads.

## Consequences

DeepSeek request translation behavior is unchanged. Diagnostics can still
identify request DTO shape without exposing prompt or metadata content.
Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite.rs`.
