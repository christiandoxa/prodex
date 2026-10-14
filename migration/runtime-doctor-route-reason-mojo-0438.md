# Runtime-doctor latest route reason migrated to Mojo

The route-health diagnosis now resolves each route's latest eligible reason
through compiled `prodex_mojo_runtime_doctor_route_latest_reason_v1`.
The previous Rust reverse chronology scan, profile/route matching and
`split_whitespace` / `strip_prefix` token parser were deleted.

Mojo validates borrowed UTF-8 event/profile/route views, scans from the
latest event backwards, requires the target profile and route, and returns
the first `reason=` or `outcome=` token's byte span from the latest
matching event containing either token. The exact legacy semantics are
preserved for empty values, case-sensitive prefixes, Unicode whitespace,
and routes whose newest event has no reason. Rust retains diagnostic
data acquisition, owned string materialization, safe offset validation,
and the other independent host-side field projections.

The Mojo core reuses the existing runtime-doctor marker owner and
route-health ABI adapter. The operation is production reachable at
`crates/prodex-runtime-doctor/src/diagnosis/route_health.rs`,
with no Rust fallback or duplicate semantic parser.

## Local evidence

- Required-Mojo direct ABI tests: 2 passed (UTF-8, empty reason, newest
  matching reason, absent route, invalid ABI, invalid span inputs).
- Required-Mojo caller route-health tests: 2 passed.
- Full required-Mojo runtime-doctor suite: 51 passed.
- Mojo ownership, authority, no-fallback, and production-share guards:
  all passed.
- The broad project production-share measure is now 32.97% Mojo;
  the long-term 75% target remains unmet. No CI-green gate was awaited.
