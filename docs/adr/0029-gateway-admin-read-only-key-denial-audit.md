# ADR 0029: Audit gateway admin read-only key mutation denials

## Status

Accepted

## Context

Gateway virtual keys can come from runtime policy or from the admin key store. Policy-backed
keys are rendered through the admin API for visibility, but they are not mutable through
admin key update or delete endpoints. Attempts to edit or delete those keys return
`403 gateway_key_read_only`.

In regulated and multi-operator deployments, these denied mutation attempts are
security-sensitive authorization events. They can indicate operator confusion,
misconfigured automation, or deliberate attempts to change policy-owned credentials
through the wrong control-plane surface.

## Decision

Read-only policy-backed key update and delete denials now append a `gateway_admin` audit
event with action `authorization_denied` and outcome `failure`. The event records the
authenticated admin actor, role, resource type, requested action, key name, backend label,
and reason `gateway_key_read_only`.

The event does not include bearer tokens or policy key token material.

## Consequences

Operators receive immutable evidence for rejected attempts to mutate policy-owned gateway
keys while preserving the existing HTTP behavior and keeping token material out of audit
logs. Policy remains the source of truth for policy-backed keys.
