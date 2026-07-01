# ADR 0706: Gateway SQLite State Path Exact Boundary

## Status

Accepted.

## Context

`gateway.state.sqlite_path` is a local filesystem path for the gateway state
store. Runtime launch previously trimmed this path before resolving it under the
Prodex root. That silently changed valid filenames with leading or trailing
spaces.

## Decision

Preserve non-blank `gateway.state.sqlite_path` values exactly at runtime.
Blank-only values remain invalid through policy validation and fall back to the
default only inside the runtime helper.

## Consequences

Operators get the exact SQLite state filename they configured. Accidental
padding is no longer hidden by runtime cleanup, making state-store routing
reviewable.
