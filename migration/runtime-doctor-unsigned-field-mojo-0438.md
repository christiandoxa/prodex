# Runtime-doctor single-field unsigned marker parsing

The remaining `runtime_doctor_marker_last_usize_field` numeric-decoding
branch is now implemented by the versioned compiled Mojo ABI
`prodex_mojo_runtime_doctor_unsigned_field_v1` in
`runtime_doctor_marker_summary.mojo`. The implementation reuses the
domain's existing unsigned decimal parser from the prior diagnostic
metric checkpoint.

Rust still reads the field from the diagnostic BTreeMap and converts the
validated Mojo output to host `usize`, but does not parse numbers, silently
recompute invalid values, or keep a semantic fallback. The operation
is production reachable through the existing doctor final-summary and
next-steps callers.

## Local verification

- Required-Mojo direct value/absence cases: passed.
- Required-Mojo ABI validation cases: passed.
- Required-Mojo production caller: passed.
- Full required-Mojo runtime-doctor library suite: 56 passed.
- Clippy `--all-targets -- -D warnings`: passed.
- Mojo ownership/authority/no-fallback/production share: PASS.
- Broad production source is 33.17% Mojo (110,584 / 333,380 LOC).
  The long-term project target of 75% is still outstanding.
