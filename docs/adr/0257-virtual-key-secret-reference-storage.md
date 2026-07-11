# ADR 0257: Virtual-Key Secret Reference Storage

## Status

Accepted

## Context

Enterprise control-plane key lifecycle requires virtual key creation and secret
rotation without storing raw key material in domain or storage plans. The
initial tenant schema already had `prodex_virtual_keys`, but it did not carry a
secret reference contract. Without a storage boundary, adapters could persist
raw virtual key secrets or miss tenant/virtual-key key validation.

## Decision

Add `VirtualKeySecretReferenceCommand` and
`plan_virtual_key_secret_reference` to `prodex-storage`. The command stores a
tenant-scoped `VirtualKeyId`, owning principal, display name, operation kind,
and `SecretRef`.

PostgreSQL and SQLite storage plans now include secret reference columns on
`prodex_virtual_keys` and expose driver-free upsert plans for virtual key
secret references. The planners reject cross-tenant storage keys and storage
keys that do not carry the same virtual key ID before adapter SQL is selected.

## Consequences

Control-plane virtual key create and rotate-secret flows can persist references
to external secret storage without raw credential material. The storage plans
remain request-path DML only; migration DDL stays in explicit migrator plans.
