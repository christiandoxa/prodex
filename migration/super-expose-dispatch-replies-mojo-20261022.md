# Super expose dispatch replies in Mojo (2026-10-22)

`parse_dispatch_request` now consumes a compiled response plan for reply kind,
audit eligibility, HTTP status and JSON-RPC code/message. Both Rust decision
matches and their reply table are removed without a fallback. Rust retains
Serde, header validation, audit effects, route acquisition and HTTP response
construction. Early notification/ID replies still precede route lookup; a
supported notification with an oversized, irrelevant tool name remains 202.

The version-1 ABI accepts one validated dispatch-decision tag and returns five
numeric metadata fields plus at most 64 message bytes. Wrong versions, invalid
tags, null/misaligned metadata and insufficient capacity reject without partial
output. Rust validates metadata, status, UTF-8 and the audited-continuation
invariant without reconstructing the decision table.

Supported notifications retain 202/null and no audit. Unsupported notifications
and invalid IDs retain 400 errors with null IDs and no audit. All five parameter
errors retain their exact 400/-32602 messages and occur after dispatch audit.
Valid requests audit before tool validation and dispatch. Earlier JSON nesting,
envelope, protocol-header and method validation preserve their precedence.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both and any archive override. Direct assertions
use `-D ASSERT=all`. Validation commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_expose_dispatch_response_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::super_expose::dispatch_response::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::super_expose::`
- `cargo test --locked -q -p prodex-app --lib super_expose::protocol::dispatch::reply_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib super_expose:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/super-wildcard-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Two real parser regressions pass before and after migration. Direct Mojo, both
new bridge tests, all 34 Super expose bridge tests and 122 Super expose caller
tests pass. Strict workspace Clippy, formatting and full default CI pass; the
CI run completes in 653.30 seconds with 1,443 Rust tests. Changed tests pass
in 4.49 seconds; documentation, guards, staged churn and diff checks pass.
The attempted
`mojo-runtime`-only test selected zero rich tests and is not positive evidence.
Standalone `mojo-rich` fails to link existing routing/JSON symbols; production
validation uses the complete `mojo-core` feature configuration. Evidence retains
these failures, direct-test integer corrections and corrected test imports.

Canonical share is 36.40900533000981% (137,302 Mojo / 239,808 Rust production
LOC). The 75% goal remains open. Native non-Linux execution and live provider
coverage remain outside local validation.
