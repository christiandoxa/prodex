# ADR 1016: Admin token key-prefix scopes fail closed

## Status

Accepted.

## Context

Configured gateway admin tokens can include `allowed_key_prefixes` to restrict
which virtual keys the control-plane credential may manage. Invalid prefix
values were silently filtered out during launch configuration. If every
configured prefix was invalid, the resulting prefix list became empty, which is
the compatibility representation for unrestricted key access.

## Decision

Admin token configuration now rejects any non-empty, whitespace-free violation
in `allowed_key_prefixes`. The parser no longer drops invalid values and
continues with a broader effective scope.

## Consequences

Malformed key-prefix scopes fail closed during gateway launch configuration
instead of widening an admin credential to all keys. Existing valid prefixes
continue to work unchanged. Regression coverage lives in
`crates/prodex-app/tests/src/app_commands/runtime_launch.rs`.
