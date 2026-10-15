# Synthetic session metadata repair moved to Mojo

`session_repair.mojo` now owns synthetic repair metadata precedence: the first
matching line ID, path-derived ID, or full selector, followed by the first
valid timestamp, CWD, and provider fields. Its bounded JSON sink preserves the
sorted `serde_json::Map` wire shape and omits an absent provider.

Rust keeps JSONL/file facts, path and selector acquisition, UTF-8 span
materialization, clock and current-directory fallbacks, package-version
projection, and the repair transaction. The production caller has no Rust
semantic fallback after a Mojo result; the typed ABI fails closed.
