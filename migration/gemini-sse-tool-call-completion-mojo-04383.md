# Gemini SSE pending tool-call completion migrated to compiled Mojo

The native Gemini GenerateContent streaming adapter now delegates selection
of unfinished tool-call slots to a compiled, versioned Mojo owner. The host
passes the existing BTreeMap's ordered completion flags, and the Mojo kernel
returns the stable positions of unfinished slots. Rust only performs the
corresponding mutable tool-call state updates and builds response items.

The old Rust completion filter and duplicate done predicate were removed.
Missing or malformed compiled output produces an explicit stream failure,
not a hidden Rust fallback decision. Already completed calls are not emitted
twice and pending calls preserve index order and their original call IDs,
signatures, and arguments.

The typed adapter validates returned positions, ensures each belongs to
an unfinished input, and rejects repeated or out-of-order entries. Direct
Mojo tests cover empty/all-completed/mixed pools, stable ordering, malformed
flags, invalid pointer shapes, short output capacity, and ABI version
mismatch before writes. A real GenerateContent SSE test verifies repeated
tool-call updates still emit exactly one terminal event per call.
The no-fallback guard rejects restoration of the old Rust completion filter.

This is a production-reachable checkpoint; the separate 75% Mojo project
target remains outstanding.
