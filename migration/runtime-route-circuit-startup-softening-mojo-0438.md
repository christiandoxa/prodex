# Runtime route circuit startup softening uses Mojo authority

The startup repair of persisted route circuit deadlines now calls
`prodex_mojo_core::runtime::profile_soften_backoff_until`, which is
already owned by compiled `prodex_runtime_health_scalar_v1`
(`RUNTIME_HEALTH_SCALAR_SOFTEN_UNTIL` in
`mojo/prodex_core/runtime_health.mojo`).

Previously Rust explicitly checked `until <= now`, removed expired
circuit entries, performed saturating `now + probe_seconds`, clipped
until, and determined whether the state changed. Those duplicate Rust
semantic decisions were deleted. Rust now resolves the route key and
health-dependent half-open probe duration, applies the Mojo keep/changed
decision, and updates the owned BTreeMap. There is no Rust fallback.

No second authoritative operation was registered: the ownership guard
requires one unique owner for the existing scalar ABI. Its previous
authority metadata remains intact and no guard was bypassed.

## Local validation

- Required-Mojo production regression covers expired route circuits,
  very long active deadlines, unknown route keys, strict boundary and
  repeated startup softening idempotence.
- Full required-Mojo runtime-proxy library suite: 436 passed.
- Clippy `--all-targets -- -D warnings`: PASS.
- Mojo ownership, authority, no-fallback and broad production-share
  guards: PASS.
- Broad production share: 33.42% (111,990 / 335,094 LOC).
  The 75% project target is still outstanding.
