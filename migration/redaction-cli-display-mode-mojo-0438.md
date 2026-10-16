# CLI redaction display-mode classification moved to Mojo

The compiled redaction owner now chooses the display mode for an already
redacted OS argument. Empty values require the existing `''` representation;
nonempty strings comprised only of ASCII letters/digits and `_-. /:=<>`
(with no spaces) remain unquoted; all other strings use Rust's host Debug
string formatter to preserve its exact escaping behavior.

The former Rust `chars().all` ASCII classifier and empty-value branch were
removed. The versioned Mojo ABI validates input UTF-8, handles zero-length
input, and returns a checked enum. Rust only materializes the Mojo-selected
output, including invoking the Rust-native Debug formatter when directed;
there is no alternate Rust classification or fallback decision.

Strict ABI tests cover the allowed punctuation, quotes, whitespace, Unicode,
malformed UTF-8 and ABI versions. Real `prodex-redaction` caller tests cover
empty, Unicode, visible characters and escaped newlines. A no-fallback guard
rejects restoring the previous Rust display classifier.

The separate 75% project production-share target remains outstanding.
