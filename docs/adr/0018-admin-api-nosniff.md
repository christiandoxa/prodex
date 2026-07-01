# ADR 0018: Add No-Sniff to Gateway Admin API Responses

## Status

Accepted

## Context

Gateway admin API responses are Prodex-owned control-plane payloads. Some responses are JSON with sensitive administrative metadata, and CSV ledger exports may be downloaded through browsers or enterprise proxies. These responses should not rely on user agents guessing content types.

## Decision

Shared gateway admin JSON and CSV response helpers now include:

```text
X-Content-Type-Options: nosniff
```

The virtual-key `ETag` response helper uses the same protection. This is scoped to Prodex admin/control-plane responses and does not modify upstream OpenAI-compatible data-plane pass-through responses.

## Consequences

Browsers and compliant intermediaries are instructed to honor the declared content type for admin responses, reducing MIME-confusion risk without changing API bodies, status codes, or streaming behavior.
