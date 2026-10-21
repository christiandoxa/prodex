# Runtime cached authentication source precedence in compiled Mojo (2026-10-21)

## Production ownership

- **Consumers:** `crates/prodex-app/src/runtime_proxy/quota/cache.rs`,
  used for cached authentication in quota selection, runtime profile
  admission, health/backoff, current route and previous-response discovery.
- **Compiled owner:** a new source-selection mode in the existing
  versioned `prodex_runtime_profile_auth_cache_policy_v1` export from
  `mojo/prodex_core/runtime_health.mojo`, through the checked
  `prodex-mojo-core/src/runtime/profile_auth.rs` adapter.
- Mojo alone selects `UsageAuth` when an auth cache entry exists (even if
  a probe also exists), otherwise `Probe` when a probe cache entry exists,
  otherwise `None`. Two separate Rust if-branches previously choosing
  precedence were deleted. The one shared Rust materializer only maps the
  compiled typed decision to `AuthSummary`; it does not recompute policy.
- Rust keeps original BTreeMap lookups, borrowed probe summaries and
  native auth data. The code clones a probe `AuthSummary` only if Mojo
  selected it, and does not send credentials or arbitrary auth bytes into
  the ABI. Invalid Mojo outputs fail without a Rust fallback.
- The existing Mojo ABI validates both boolean presence facts before
  writing its output; the Rust enum decoder rejects unexpected tags.

## Regression

- Direct Mojo policy fixture tests all four combinations of usage/probe
  presence, malformed flags, wrong ABI versions and error output untouched.
- Rust bridge source precedence test and live quota-cache caller test cover
  original `chatgpt` label, quota compatibility, probe label, none and
  usage-auth dominance over a conflicting probe.
- Full workspace Clippy all targets/features with `-D warnings`, Mojo
  no-fallback/authority, size, formatting and source-share guards pass.
- Audited Mojo share after the change is 36.17464732817056%
  (134,858 Mojo / 237,939 Rust production LOC), a minor decrease from
  36.17681698310114% due to the host adapter. The separate 75% target
  remains unmet.
