# ADR 0683: Gateway Route Alias Names Match Exactly

## Status

Accepted.

## Context

Gateway route alias names are routing policy identifiers. Runtime config
resolution previously trimmed alias names, and policy validation only rejected
trim-empty values. A padded alias name could therefore be normalized into an
active route alias.

## Decision

Route alias names must now be exact non-empty values without whitespace. Runtime
config resolution no longer trims alias names; policy files fail closed when an
alias name contains whitespace.

## Consequences

Canonical alias names are unchanged. Padded or whitespace-bearing alias names no
longer become active routing policy through trim-normalization.
