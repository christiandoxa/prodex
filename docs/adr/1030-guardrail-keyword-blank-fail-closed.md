# ADR 1030: Guardrail blank keyword entries fail closed

## Status

Accepted.

## Context

Gateway guardrail keyword lists are security policy. Runtime resolution already
preserves non-blank keyword text exactly, but blank-only entries were silently
dropped. That hid malformed policy and made the active guardrail set differ
from the configured policy without operator feedback.

## Decision

`gateway.guardrails.blocked_keywords` and
`gateway.guardrails.blocked_output_keywords` now reject blank-only entries
during runtime launch. Non-blank keyword values are still preserved exactly.

## Consequences

Operators see malformed guardrail keyword configuration at startup instead of
silently running with a different guardrail set. Deployments that want no
keyword guardrail should omit the entry rather than configure a blank value.
