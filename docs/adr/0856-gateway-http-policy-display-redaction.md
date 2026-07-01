# ADR 0856: Redact gateway HTTP policy display output

Status: Accepted

## Context

Gateway HTTP policy validation errors distinguish body limits, timeout budgets,
concurrency limits, and drain timeout configuration. Their `Display` output
named those local configuration knobs directly.

## Decision

Keep typed policy error variants for validation and response planning, but
render local display output as a generic gateway HTTP policy failure.

## Consequences

Policy validation behavior is unchanged, while stringified local policy errors
no longer expose internal configuration topology.
