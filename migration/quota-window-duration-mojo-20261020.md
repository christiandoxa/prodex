# Quota Window Duration Normalization Migrated to Mojo (2026-10-20)

## Ownership

The compiled quota owner now normalizes optional usage-window durations
through versioned ABI prodex_quota_window_duration_seconds_v1.
An explicitly present seconds value takes precedence even when zero,
negative, or at the signed 64-bit boundary. Otherwise a present minutes
value is multiplied by 60 using checked signed arithmetic. Overflow
produces absence. Serde still acquires the input provider fields.

The previous Rust optional-field fallback and minutes multiplication
were deleted, with no Rust semantic fallback. The typed adapter
validates ABI version, presence and result tags.

## Evidence

The complete 100-test quota suite, two direct core ABI tests and the
direct Mojo test pass. Cases include explicit seconds precedence,
negative and zero values, 300/10080 minutes, signed boundaries,
overflow and invalid flags/ABI versions. Anti-fallback guard, formatting
and audited production-share checks pass.

Canonical share increases from 35.826879696153064% to
35.828561551188166% (130,584 Mojo / 233,885 Rust production LOC).
Target 75% is still unmet. No counting rules or release floor changed.
