# Nested Gemini Media Traversal Migrated to Mojo (2026-10-20)

## Production ownership

The compiled Gemini value owner now enumerates media-candidate objects
via the versioned MediaObjectIndices JSON-tree operation. The kernel
walks arrays in their original order and visits each object before its
first-present content member; other object members do not create
traversal edges. The old recursive Rust media collector was deleted.
Rust only acquires a borrowed structural tree, translates the compiled
object indices to the original Serde object references, and invokes the
previously migrated Mojo-based media field selector and callback handler.

The new shared Serde host method, Document::borrowed_shape, stores flat,
borrowed JSON node metadata and original Value references. It does not
clone or serialize large image/audio/video byte strings, preserves
original source lifetime, and uses an iterative stack only for generic
JSON structure acquisition. Business traversal ordering and eligibility
are determined solely by compiled Mojo, not recomputed by Rust.

## Validation

- Three caller regressions cover nested arrays, object-before-content
  order, ignored unrelated members, local path callback ordering, a
  256-level array, and a borrowed 1 MiB media string.
- Two additional compiled core ABI tests cover content-only traversal
  and a deeply nested flat tree with 4,097 objects without recursion.
- All 523 strict provider-core tests and nine direct Gemini value ABI
  tests pass.
- The Mojo no-fallback guard rejects the retired recursive Rust call
  and requires the new borrowed shape and compiled object-index ABI.
- Cargo format, diff, and official reachable-Mojo non-regression checks
  pass. Remote CI was not a prerequisite.

Canonical broad share increases from 35.77956915288745% to
35.78094701085088% (130,087 Mojo / 233,478 Rust lines). The
75% project target remains open; no counting rules or release floor
were changed.
