# Runtime Thread-State Database Priority Migrated to Mojo (2026-10-20)

## Compiled production ownership

The existing compiled runtime_thread_index.mojo owner now chooses
database search priority among filtered state_*.sqlite candidates.
The versioned prodex_runtime_thread_index_database_order_v1 ABI
sorts original borrowed UTF-8 filename views in lexical descending
order, breaking identical filename ties by stable original position.

This preserves the former behavior, including state_9.sqlite taking
precedence over state_10.sqlite (lexical, not numeric priority),
Unicode filenames, and stable exact duplicates.

Rust retains filesystem directory enumeration, UTF-8 filename
acquisition, state_*.sqlite filtering, regular-file and symlink
checks, SQLite read-only database I/O, and application of
validated Mojo-selected indices to original PathBuf values.
The previous paths.sort_by comparison has been deleted without
a Rust semantic fallback.

## Verification

- All 15 prodex-app runtime_thread_index module tests pass,
  including two new real caller regressions for a shared thread
  appearing in both state_9.sqlite and state_10.sqlite and
  exclusion of symlinks, directories, and invalid filenames.
- Two direct compiled-core tests cover descending Unicode
  names, stable equal ties, 1,025 candidates, oversized
  filename rejection, malformed late views, invalid UTF-8
  and untouched outputs after rejected ABI calls.
- Direct Mojo ABI regression, source anti-fallback guard/self-test,
  Cargo formatting and official production-share non-regression
  checks pass.

Canonical share increases from 36.01500636576116% to
36.02408461634476% (132,761 Mojo / 235,773 Rust production LOC).
The 75% project target remains unmet. Source counting,
historical release floors and no-fallback requirements were
unchanged. Remote CI green was not a prerequisite.
