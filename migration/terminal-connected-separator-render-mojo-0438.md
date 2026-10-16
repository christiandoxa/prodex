# Terminal connected separator UTF-8 rendering in Mojo

The production terminal UI and quota-watch frame now render connected panel
separators through the existing compiled terminal layout owner. Mojo owns
all four visual cases: zero width, one horizontal glyph, the two end caps,
and the repeated middle horizontal glyphs for wider panels. The compiled
ABI writes the exact UTF-8 box-drawing bytes to caller-owned storage.

Rust now only delegates to Mojo, validates the returned byte count and UTF-8,
and returns the already-rendered string. Its previous width match, repeat
count and box-drawing formatter were deleted with no Rust fallback.

Direct compiled-Mojo tests verify widths 0, 1, 2, 3, 5, 10 and the maximum
u16 width, and fail-closed invalid ABI version, capacity and pointers.
Caller tests exercise tiny and maximal panel widths. The source guard prevents
restoring the old Rust renderer. This checkpoint does not meet the separate
long-term 75% production-share target.
