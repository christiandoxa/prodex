# ADR 0498: Preserve W3C trace context for OpenAI-compatible transport

## Status

Accepted

## Context

The local rewrite proxy should stay transport-transparent while adding gateway
credential handling. OpenAI-compatible chat-completions forwarding already
preserves most caller metadata and replaces only the upstream authorization
credential.

Enterprise observability requires end-to-end W3C trace propagation so downstream
provider calls can be correlated with the original gateway request.

## Decision

OpenAI-compatible transport preserves incoming `traceparent` and `tracestate`
headers when forwarding chat-completions requests. The proxy still replaces
`Authorization` with the selected upstream API key.

This is covered by the characterization test
`openai_compatible_transport_preserves_trace_context_for_chat_completions`.

## Consequences

Gateway traces remain correlated across the caller, proxy, and upstream provider
boundary without exposing caller data-plane credentials to the provider.
