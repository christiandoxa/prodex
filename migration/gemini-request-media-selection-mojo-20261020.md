# Gemini Request-Media Field Selection Moved to Mojo (2026-10-20)

## Production ownership

The existing compiled Gemini response-media owner now also decides field
precedence for request media objects via the versioned
prodex_mojo_gemini_media_request_selection_v1 ABI. The host acquires
22 borrowed, typed Serde fields into a fixed-size scalar table, without
serializing, cloning or copying large image/audio/video payloads.

Mojo owns all of the following decisions:
- A direct image URL string versus the first-present URL/image_url field,
  including non-string presence barriers.
- Request media kind (image, generic file, audio, video or unsupported).
- Nested image URL candidate precedence and fallback to the outer URL.
- The first-present MIME, base64/data, URI and local-path fields, with
  non-string field blocking later aliases.
- Unicode-whitespace-only data falling through to URI, and string URI
  presence (even empty) preventing the local-path callback.

Rust retains only Serde field lookup (host acquisition), the typed
ABI call, input/result bounds validation, and the application of the
compiled action to existing data/URI converters and local-path callbacks.
The original Rust media kind dispatch, image source selection and nested
generic MIME/data/URI/path fallback algorithm were deleted. The ABI
contains no Mojo runtime heap allocation and does not copy large media.

## Verification

- Compiled Mojo module links into the production archive.
- All 517 strict provider-core tests pass, including three new live
  request-media caller tests for nested images, first-present null/type
  barriers, Unicode data, URI precedence, and local callback behavior.
- Three direct Mojo-core ABI tests pass, including malformed version,
  pointer/row-shape inputs rejected before result writes.
- Mojo no-fallback self-test and source ownership checks pass.
- Cargo format, diff checks, and official production-share non-regression
  validation pass.

The canonical production-share metric rises from 35.76609372332399% to
35.7782309443018% (130,007 Mojo / 233,362 Rust production lines).
The 75% project goal remains unmet. No counting/denominator edits or
fallback implementations were introduced.
