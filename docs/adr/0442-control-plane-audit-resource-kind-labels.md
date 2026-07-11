# ADR 0442: Use stable control-plane audit resource labels

## Status

Accepted.

## Context

Control-plane audit resources used Rust `Debug` output for `ResourceKind`.
That made audit logs depend on internal enum spelling.

## Decision

Control-plane audit resources now use stable snake_case labels that match the
domain serialization shape.

## Consequences

Audit records avoid leaking Rust enum names and remain stable across internal
renames.
