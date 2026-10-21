# Runtime profile usage-auth refresh and HTTP backoff scoring in Mojo (2026-10-21)

## Production ownership

- **Consumers:** `crates/prodex-app/src/runtime_persistence/auth.rs`,
  including cached usage-auth reads in the live runtime and HTTP auth-failure
  accounting used by profile rotation, throttling and reload.
- **Compiled owner:** `mojo/prodex_core/runtime_health.mojo` exposes versioned
  `prodex_runtime_profile_auth_cache_policy_v1` with two operation tags.
  The checked Rust bridge lives in
  `crates/prodex-mojo-core/src/runtime/profile_auth.rs`, re-exported by
  `prodex_mojo_core::runtime`.
- The fresh/stale cache decision now belongs exclusively to compiled Mojo:
  detects backwards clock jumps, saturating elapsed time reaching the
  configured exact TTL, and existing quota-auth proactive refresh requests.
  The previous native Rust comparisons and boolean chain have been removed.
- The HTTP auth-failure score also belongs exclusively to compiled Mojo:
  HTTP 401 receives the configured unauthorized score, all other statuses
  receive the existing alternate score, and the recorded value never falls
  below the currently effective health score. The previous Rust `match`
  and `.max` scoring branch were removed.
- Rust remains responsible for locks, auth JSON reads, profile resolution,
  existing score lookup/decay, configured test/production constants, saving
  health state and scheduling background refreshes. No credential or API
  payload crosses the scalar ABI. Unknown ABI responses fail without a
  recomputed Rust policy.

## Offline regression

- Direct Mojo ABI fixture covers cache expiry boundaries, clock rollback,
  proactive refresh, status 401/403, retention of greater prior score,
  malformed flags, ABI versions and output non-mutation on error.
- Two compiled bridge tests cover timestamp `i64::MIN/MAX`, negative TTL,
  and configured scores including `u32::MAX`.
- Two live runtime-auth cache tests and five profile-auth-failure app tests pass
  with offline local fixtures.
- Full workspace Clippy (`--locked --workspace --all-targets --all-features
  -- -D warnings`), `cargo fmt`, the Mojo no-fallback/authority guards,
  source-share, size guard and documentation lint pass.
- Official source share: 36.17422107720795% before this checkpoint to
  36.17681698310114% (134,848 Mojo LOC / 237,899 Rust LOC).
  The project 75% target remains separate and **not yet met**.
