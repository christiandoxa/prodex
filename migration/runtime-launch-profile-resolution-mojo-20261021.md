# Runtime launch profile resolution and monotonic selection in Mojo (2026-10-21)

- **Production consumer:** `prodex-runtime-launch/src/profile.rs`, called by
  profile-aware application launch, quota inspection, and profile-management paths.
- **Compiled owner:** `mojo/prodex_core/profile_schedule.mojo` exports
  `prodex_runtime_profile_name_resolution_v1` and
  `prodex_runtime_profile_next_selection_at_v1` through the checked
  `prodex-mojo-core/src/runtime/profile_launch.rs` bridge.
- Rust retains `AppState` map lookups, pruning, persistent updates, and user
  error materialization. Mojo alone chooses requested/active/single profile
  precedence, missing-profile categories, and monotonic selection timestamps.
  The previous Rust branching and timestamp max/saturating-add logic are
  removed; no feature-off or error fallback recomputes them.
- The timestamp ABI reads bounded borrowed i64 observations, retains the
  historical zero default when all observations are negative, and saturates
  on i64::MAX. The tagged profile outcome is checked before use.
- **Regression coverage:** direct Mojo ABI test with empty/negative/saturated
  timestamps, valid outcomes and invalid inputs; direct Rust bridge tests;
  caller tests for stale-active/requested error precedence, single profiles,
  deleted timestamps, negative epochs, and saturation.
- **Scope:** no changes to provider traffic, credential handling, runtime
  affinity, release metadata, production metric exclusions, or historical
  counting baselines. The 75% production-source target remains independent
  from the enforced 7% release floor.

## Validation at checkpoint

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/profile_launch_policy_test.mojo`: PASS.
- `cargo test --locked -q -p prodex-runtime-launch`: 77 passed.
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core profile_launch`: 2 passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`: PASS.
- Mojo anti-fallback, authority, size, production share, Markdown and
  `git diff --check`: PASS.
- Measured official share: 36.11800898926088% -> 36.120514009120214%
  (134,023 Mojo / 237,021 Rust production LOC). The 75% goal is still unmet.
