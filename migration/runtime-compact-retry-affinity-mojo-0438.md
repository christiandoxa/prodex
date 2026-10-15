# Compact retry affinity matching migrated to Mojo

Compact retry decisions now call the versioned
`prodex_runtime_compact_retry_decision_v3` entrypoint in
`mojo/prodex_core/quota_pressure.mojo`. Mojo compares the candidate with the
compact-followup, previous-response, and session owners, while keeping the
current-profile overload retry separate from hard affinity. Rust retains
profile and request state acquisition, retry side effects, transport handling,
and typed ABI mapping; the former Rust hard-affinity computation was deleted.

The v3 ABI validates its version, borrowed profile views, presence tags, and
retry state. Required-Mojo regressions cover missing/current/foreign owners,
Unicode and long names, invalid ABI input, committed no-rotation behavior, and
the existing quota/overload failover order. The compact retry source is absent
from the frozen counted Rust inventory, so its ownership record claims no
migration volume.

## Local verification

- Required-Mojo `prodex-mojo-core` quota-decision tests: 7 passed.
- Required-Mojo compact app regressions: 29 passed; runtime smoke suite passed.
- Focused Mojo-core and prodex-app Clippy checks, ownership, authority,
  no-fallback, hot-path, manifest, compatibility replay, formatting, and diff
  checks passed.
- Broad production share: 33.48% Mojo (112,382 / 335,711 LOC); the 75% project
  target remains outstanding.
