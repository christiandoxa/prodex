# ADR 1021: Guardrail webhook phases fail closed

## Status

Accepted.

## Context

Gateway guardrail webhooks can be limited to request (`pre`) and response
(`post`) phases. Invalid phase strings were silently filtered or preserved as
unknown phases during launch configuration. A malformed configured phase could
therefore disable an intended webhook check.

## Decision

Guardrail webhook phases now fail closed. Configured phases must be exact
`pre`, `post`, `request`, or `response` values without whitespace. The aliases
`request` and `response` are normalized to `pre` and `post`; invalid values
reject gateway launch configuration.

## Consequences

Malformed webhook phase configuration can no longer skip intended guardrail
checks. Deployments must remove absent phase restrictions or provide exact phase
values. Existing valid phase values and aliases continue to work. Regression
coverage lives in `crates/prodex-app/tests/src/app_commands/runtime_launch.rs`.
