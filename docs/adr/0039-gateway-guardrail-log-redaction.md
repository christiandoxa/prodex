# ADR 0039: Gateway guardrail diagnostic logs redact matched values

## Status

Accepted

## Context

Gateway guardrails can block prompts, buffered responses, streaming responses, and guardrail webhook decisions. Previous audit hardening ensured immutable audit events only recorded metadata such as action, path, backend, and reason. Runtime diagnostic logs, however, still included the matched guardrail `value` for several denial paths.

Matched values can contain user prompt fragments, model output, policy-sensitive keywords, or webhook decision details. In regulated multi-tenant deployments, runtime logs may be shipped to shared observability infrastructure, so they must not include raw prompt/output snippets or policy internals.

## Decision

Runtime guardrail denial logs remain structured and operationally useful, but replace raw matched values with `matched_value_redacted=true`.

This applies to:

- request keyword guardrail blocks;
- webhook guardrail blocks;
- buffered response output guardrail blocks;
- post-response webhook guardrail blocks;
- streaming output guardrail blocks.

Audit events continue to omit matched values and bearer credentials.

## Consequences

Operators can still identify that a guardrail blocked traffic and see request id, transport/phase, and denial reason. They cannot recover the exact matched prompt/output from runtime logs, reducing accidental leakage across tenants and log aggregation systems.

Troubleshooting exact policy matches must use controlled tenant-owned reproduction or future purpose-built secure evidence workflows rather than generic runtime logs.

## Validation

Regression tests assert that request and buffered response guardrail denials:

- produce expected audit events;
- include guardrail denial metadata in runtime logs;
- include `matched_value_redacted=true` in runtime logs;
- do not write gateway tokens or matched prompt/output strings to audit or runtime logs.
