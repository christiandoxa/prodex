# ADR 0980: Gateway compatibility state persists typed virtual key IDs

## Status

Accepted

## Context

The gateway compatibility key store still identified admin-managed virtual keys
primarily by display name plus governance scope. That was enough for legacy
CRUD and budget checks, but it left no typed `VirtualKeyId` to bridge admin
state into the tenant-keyed durable accounting schema.

Without a persisted key ID, the planned migration from local compatibility
admission to reservation-backed accounting would keep inventing adapter-local
identity mapping or fall back to tenant-only counters.

## Decision

Persist an optional canonical `virtual_key_id` in the gateway compatibility key
store across file, SQLite, PostgreSQL, and Redis backends.

- New admin-created keys receive a UUIDv7 `VirtualKeyId` at creation time.
- Compatibility migration version `3` adds the SQL column for SQLite and
  PostgreSQL stores.
- Compatibility loaders treat malformed or non-canonical values as invalid and
  reject that stored key entry instead of trim-normalizing it.
- Admin API payloads now expose `virtual_key_id` so later reservation-backed
  admission can reuse the same tenant-owned identifier.

## Consequences

The compatibility layer now has a stable typed key identity that lines up with
the durable accounting schema. This does not wire reservation execution by
itself, but it removes one of the remaining identity gaps before that adapter
switch.
