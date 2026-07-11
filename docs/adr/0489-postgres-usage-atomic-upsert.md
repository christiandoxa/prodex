# ADR 0489: Apply Postgres Gateway Usage Deltas with Atomic Upserts

## Status

Accepted.

## Context

Gateway Postgres usage persistence read the current virtual-key usage row, updated counters in application memory, then wrote the replacement row. Under multi-replica writes, that shape can lose usage when two writers race around the same counter state.

## Decision

Postgres gateway usage deltas now use one `INSERT ... ON CONFLICT(key_name) DO UPDATE` statement. The conflict update increments total request, minute token, and spend counters in SQL, and resets minute counters when the delta minute changes.

## Consequences

Postgres usage accounting no longer depends on an application read-modify-write cycle for counter updates. This is still delta accounting, not the final reservation-based accounting model.
