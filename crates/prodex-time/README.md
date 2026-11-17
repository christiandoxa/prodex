# Prodex host time boundary

This crate replaces the unmaintained registry `chrono` dependency with pinned
Jiff 1.0.0. The workspace retains `chrono` as a dependency alias solely for the
small existing host API surface; no Chrono implementation is copied, vendored,
or linked. New calendar/timezone behavior belongs to Jiff, not this adapter.

The boundary preserves persisted offset/fraction formats, negative Unix epochs,
RFC 3339/2822 parsing, and DST rules: gaps return no value; overlaps are ambiguous
unless the caller explicitly chooses the earlier instant. Unsupported or
out-of-range civil dates fail instead of wrapping. Jiff supports years -9999
through 9999; invalid extremes retain caller fallbacks.

The adapter is not a general Chrono replacement. Its API is limited to existing
Prodex host callers, and business policies remain in their original Mojo owners.
The registry advisory is not ignored: `deny.toml` remains unchanged and the
release dependency graph must not contain registry `chrono`.

Source: [RustSec RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543.html)
and [Jiff](https://github.com/BurntSushi/jiff).
