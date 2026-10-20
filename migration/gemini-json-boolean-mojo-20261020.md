# Gemini JSON Boolean Policy in Compiled Mojo (2026-10-20)

## Ownership and compatibility

The production Gemini value JSON-tree kernel now owns classification of
JSON Boolean, integer, string, null, array, and object values. The previous
Rust match converting Serde JSON values to optional booleans was deleted:
Rust merely acquires a Serde JSON document, calls the versioned compiled
Mojo projection, and decodes its result.

The numeric path preserves Serde signed-integer conversion semantics:
signed in-range integers other than zero are true; zero,
fractional/exponent numbers, and unsigned integers outside i64 are false.
String values reuse the original provider Boolean-token classifier, now
extracted into a private shared provider_boolean_tokens.mojo helper.
Both the provider scalar-policy entrypoint and the Gemini JSON kernel
reference the same helper. The previous duplicate string-token table in
the provider scalar module was deleted. The shared helper does not export
a second C ABI or duplicate the versioned provider symbol.

## Focused validation

- Both Mojo source modules compile as objects.
- Strict provider-core library suite: 514 tests pass, including direct
  live JSON Boolean input cases for signed bounds, floating point,
  out-of-range unsigned, Unicode whitespace, and malformed tokens.
- Strict core Gemini value ABI: 7 tests pass.
- Existing provider scalar policy tests pass.
- No-fallback guard self-test passes, with shared Boolean helper checks.
- Cargo format, diff validation, and official Mojo production-share
  non-regression checks pass.

The canonical broad share moves from 35.75250743965612% to
35.75917369898641% (129,794 Mojo / 233,173 Rust production lines).
The 75% project target is unmet. Source counting, non-regression and
the release floor have not changed. Remote CI was not a blocking gate.
