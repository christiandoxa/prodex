# Parallel Ping Result Profile Ordering Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled ping_protocol.mojo owner now sorts all
completed OpenAI ping results by original profile-name UTF-8
bytes through versioned prodex_mojo_ping_profile_order_v1.
It returns the original source indices in ascending lexical order,
with stable original position when names compare equal, preserving
the prior sort_by behavior after nondeterministic parallel worker
completion.

Rust retains worker queues, subprocess execution and latency
measurements, original PingResult ownership, and application
of the Mojo-selected permutation to those original results.
Its previous results.sort_by profile comparator has been
removed, with no Rust semantic fallback.

## Verification

- All 25 prodex-app ping tests pass, including two new
  production caller regressions for Unicode names, duplicate
  equal-name results with different details and 1,025
  reordered worker completions.
- Two direct core ABI tests pass, including 1,025 source names,
  Unicode lexical order, empty strings, malformed late UTF-8,
  bad version/capacity, and unchanged output on rejected calls.
- Direct Mojo ABI test, source no-fallback guard/self-test,
  Cargo formatting and canonical Mojo production share
  non-regression checks pass.

Official broad share rises from 36.049461001902344% to
36.05829134839374% (133,120 Mojo / 236,060 Rust production LOC).
The separate 75% project target remains unmet; accounting and
historical release-floor rules are unchanged. Remote CI green
was not a prerequisite to publication.
