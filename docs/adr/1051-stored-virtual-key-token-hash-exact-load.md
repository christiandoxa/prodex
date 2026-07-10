# ADR 1051: Stored virtual-key token hashes load exactly

## Status

Accepted.

## Context

Admin-managed virtual keys store only a base64-encoded SHA-256 bearer-token
hash. Compatibility key-store loads decode that persisted hash before a virtual
key can become active, but the shared decoder accepted leading or trailing
whitespace by trimming the stored value.

Affected symbol:

- `LocalBridgeBearerTokenHash::from_hash_base64`

The risk is credential-state drift: corrupted persisted credential fields can
load successfully even though exact secret handling elsewhere rejects
whitespace-bearing credential material.

## Decision

Bearer-token hash base64 decoding no longer trims the input. Persisted hashes
must be exact base64 for the 32-byte SHA-256 digest.

## Consequences

Canonical persisted hashes continue to load. Whitespace-bearing or otherwise
non-canonical hash fields fail closed before a stored virtual key can become an
active data-plane credential.

Regression coverage:

- `bearer_token_hash_base64_decode_is_exact`
- `stored_key_rejects_padded_token_hash`
