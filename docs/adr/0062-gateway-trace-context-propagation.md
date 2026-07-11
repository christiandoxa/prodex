# ADR 0062: Gateway trace-context propagation

## Status

Accepted.

## Context

The enterprise target requires end-to-end trace context propagation across the
gateway data plane and provider calls. The local rewrite transport already
preserved inbound headers for OpenAI-compatible upstreams, but provider adapters
that synthesize their own wire headers (Copilot, Anthropic, DeepSeek, and Gemini)
did not forward W3C trace context. That made provider latency and reconciliation
events harder to correlate with caller traces.

## Decision

Forward the W3C `traceparent` and `tracestate` headers from the captured gateway
request to every local rewrite upstream request after provider-specific auth and
required wire headers are applied. Do not add these values as metric labels; they
remain transport/log correlation data only.

## Consequences

- Caller trace context can reach provider adapters without weakening auth header
  replacement or hop-by-hop header filtering.
- OpenAI-compatible paths keep their existing metadata preservation behavior.
- A later OpenTelemetry integration can bind these headers to native spans while
  preserving this compatibility behavior.
