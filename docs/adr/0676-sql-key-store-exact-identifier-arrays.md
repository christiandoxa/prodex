# ADR 0676: SQL Key Store Loads Identifier Arrays Exactly

## Status

Accepted.

## Context

Gateway SQLite and PostgreSQL key stores persist allowed model IDs and SCIM key
prefixes as JSON arrays. Admin write paths reject padded model IDs and key
prefixes, but SQL load paths parsed those arrays directly. A manually edited or
corrupted SQL row could therefore reintroduce whitespace-bearing identifiers
into active runtime state.

## Decision

SQLite, PostgreSQL, and Redis key-store loaders now use the same exact JSON
vector parser for identifier arrays. Every entry must be non-empty and contain
no whitespace; otherwise the array is ignored.

## Consequences

Canonical persisted identifier arrays are unchanged. Padded or empty array
entries no longer become active model policies or SCIM key-prefix scopes.
