# DeepSeek streaming continuation normalization migrated to Mojo

Two existing production-reachable DeepSeek rules have been migrated to
the compiled DeepSeek stream policy owner.

- Fallback response-ID eligibility now checks the exact
  resp_{provider}_ byte prefix in Mojo, protecting ownership of an upstream
  response ID once it has replaced the provisional generated one.
- Streamed tool-call delta handling now delegates four decisions to Mojo:
  existing/visible function object eligibility, incremental argument
  fragment presence, optional argument delta presence, and
  Unicode-whitespace-only thought signature elision.

The previous Rust prefix formatter, starts_with gate, Boolean tool-call
eligibility, argument empty string checks and Unicode trim checks were
removed. Rust remains responsible for serde_json object access and
typed output assembly, not for the migrated decisions.

Direct compiled Mojo tests cover exact fallback prefixes with Unicode,
all four delta modes, malformed mode/version/flags and nonblank Unicode.
The real provider-core caller regression preserves response-ID replacement,
malformed tool call errors, incremental argument bytes and existing
session continuation behavior. No semantic Rust fallback remains.

The 75% broad production Mojo-share target remains outstanding.
