# ADR 0698: Guardrail Webhook Phase Rejects Whitespace Padding

## Status

Accepted.

## Context

`gateway.guardrails.webhook_phases` controls whether the guardrail webhook runs
before requests, after responses, or both. Policy validation and direct runtime
config resolution trimmed phase values before matching `pre`, `post`, and their
aliases.

## Decision

Guardrail webhook phase values must be exact non-empty values without
whitespace. Policy validation rejects whitespace-bearing phases, and direct
runtime config resolution no longer trim-normalizes padded phase values into
active webhook phases.

## Consequences

Canonical phases `pre` and `post`, plus aliases `request` and `response`,
remain valid. Padded phase values no longer alter when guardrail webhooks run.
