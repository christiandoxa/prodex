# @christiandoxa/prodex

This package is the npm launcher for the native `prodex` binary.

It mirrors the published Rust crate version from crates.io and uses the same
native-binary pattern as `@openai/codex`: npm resolves the right platform
package, launches the native `prodex` binary, and injects a `codex` shim so the
Rust binary can delegate to `@openai/codex` without requiring a separate global
install.

The npm package version is intended to stay aligned with the published
`prodex` crate version.
