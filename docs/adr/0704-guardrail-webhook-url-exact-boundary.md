# ADR 0704: Gateway Guardrail Webhook URL Exact Boundary

## Status

Accepted.

## Context

`gateway.guardrails.webhook_url` routes request and response guardrail checks to
an external service. The runtime launch helper previously trimmed the configured
URL, and policy validation also trimmed before URL validation. A padded URL could
therefore be silently normalized into a different accepted target.

## Decision

Treat `gateway.guardrails.webhook_url` as an exact configuration boundary. The
value must be non-empty, contain no whitespace, and parse as an HTTP(S) URL with
a host. Runtime launch preserves accepted values instead of normalizing them and
applies the same validation when handed already-built policy settings.

## Consequences

Operators must correct padded webhook URL values in `policy.toml`. This keeps
guardrail routing auditable and avoids hidden cleanup at the security boundary.
