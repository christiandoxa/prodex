# Domain Error Response Mapping Migrated to Compiled Mojo (2026-10-20)

## Ownership and security boundary

The production domain error kernel, domain_error_policy.mojo, owns the
response status, error-code selection and public message classification
for invalid domain identifiers, invalid secret rotation policy and
secret resolution failures. The ABI is versioned, returns three validated
integer tags, and receives only trusted identifier-kind labels or enum
discriminants. It never receives a raw identifier, UUID, key, secret
reference, provider token or other secret material.

The typed prodex-mojo-core adapter translates the compiled tags into
the existing public response labels and enum types. Rust retains UUID
generation/parsing, typed public domain errors, Serde serialization,
and the effect-free construction of the typed response value. The prior
Rust match tables for identifier codes and secret error statuses/codes/
messages have been deleted rather than retained as fallback policy.
Unrecognized ABI tags and malformed arguments fail explicitly.

## Verification

- Compiled Mojo domain-error ABI: 3 direct core tests pass, including
  exact case-sensitive identifier classifications, all secret variants,
  unsupported versions, malformed fields and unchanged output buffers.
- All prodex-domain tests pass: identifier, secret, governance and
  doctest suites. New live caller tests exhaust all ten identifier
  variants and both secret rotation plus four secret resolution cases.
- Caller tests assert no raw invalid identifier or secret material is
  included in serialized or formatted responses.
- Mojo no-fallback guard and self-test, Cargo formatting, diff check,
  and canonical production-share non-regression checks pass.

The official broad share increases from 35.78094701085088% to
35.78715858637232% (130,174 Mojo / 233,571 Rust production lines).
The 75% project target remains unmet. Counting definitions, baseline and
release floor remain unchanged. No remote CI green prerequisite.
