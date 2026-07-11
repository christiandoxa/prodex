# ADR 0049: Gateway admin key-store load errors use stable messages

## Status

Accepted

## Context

Gateway admin key mutations load the persisted virtual-key store before applying create, update, rotate, disable, or delete operations. File-backed store load failures could return raw JSON parser or filesystem error strings through admin API responses.

Raw parser and filesystem messages can expose implementation details such as line/column offsets, expected tokens, absolute paths, or local environment details. The enterprise API-governance target requires stable machine-readable error envelopes and non-leaking control-plane responses.

## Decision

At the admin API boundary, file-backed key-store load errors now use stable messages:

- `gateway_key_store_invalid` => `gateway key store is invalid`
- `gateway_key_store_load_failed` => `gateway key store could not be loaded`

The stable error codes and HTTP `500` status are preserved.

## Consequences

Operators and clients still receive a deterministic failure code while parser/filesystem internals are withheld from the API response. Detailed local diagnostics should be obtained from controlled operational inspection rather than client-facing control-plane errors.

## Validation

A regression test corrupts the file-backed key store after gateway startup, invokes an authenticated admin key-create mutation, and verifies that:

- status is `500`;
- error code is `gateway_key_store_invalid`;
- message is stable;
- parser details, corrupt file content, and the admin token are absent.
