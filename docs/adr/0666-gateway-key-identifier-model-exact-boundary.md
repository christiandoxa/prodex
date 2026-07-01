# ADR 0666: Gateway Key Identifier and Model Exact Boundary

## Status

Accepted.

## Context

Gateway admin key creation accepts request-controlled virtual-key names, and key
create/update accepts allowed model IDs. These values affect authorization,
routing, and budget policy. Trimming them can turn a malformed request into a
different accepted key name or model allow-list entry.

## Decision

Gateway key names and allowed model IDs must be exact non-empty strings without
whitespace. Key names continue to use the existing stable ASCII character set.
Allowed model IDs reject empty or whitespace-bearing values instead of trimming
them before persistence.

## Consequences

Padded key names fail the existing `invalid_gateway_key_name` response. Padded
or whitespace-bearing allowed model IDs fail `invalid_allowed_models`. Clients
must send canonical key names and model IDs explicitly.
