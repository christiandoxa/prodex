# Codex 0.162.0 streamed retry-advice qualification

## Reproduced production-boundary defect

The existing Mojo error classifier correctly recognized `rate_limit_exceeded`,
but the proxy derived the retry delay only from the error message. A regression
using `response.failed.response.error.headers.Retry-After = "5"` together with
`message = "Please try again in 1s."` failed before the patch: actual one second,
expected five seconds. The same test passes through the patched production path.

## Ownership and transport contract

`prodex_mojo_runtime_retry_after_json_v1` in the reachable `rich_fallback.mojo`
root owns JSON header paths, nested/outer precedence, header name/value validity,
JSON-map insertion semantics, numeric advice, zero, and the existing 300-second
local delay cap. It receives a typed borrowed JSON tree, not a second raw-JSON
semantic implementation. Rust supplies serde decoding, owned buffers, one wall
clock sample, and the existing `httpdate` codec. Date intervals round upward to
milliseconds so ABI conversion cannot make a retry deadline earlier.

For `response.failed`, advice comes from `response.error.headers`. For a
WebSocket `error` wrapper, usable nested `error.headers` advice wins, followed by
outer `headers`, then the existing message-derived advice. Different-case JSON
header names follow upstream `HeaderMap::insert` iteration order; they are not
combined with the maximum rule used for repeated transport header entries.
Unsupported JSON values and invalid HTTP header values are ignored. A valid HTTP
value containing invalid retry advice can replace an older same-name value,
exactly as in the upstream JSON-to-header conversion.

Both `parse_runtime_sse_event` and the real WebSocket frame inspector invoke the
Mojo plan through `runtime_stream_error_policy_from_value`. Raw WebSocket JSON
bytes take the same path. Tests prove the rate-limit scheduler receives five
seconds and that a committed WebSocket frame does not request retry or rotation.
This does not redesign generic overload/profile-rotation scheduling: its existing
policies, hard-affinity rules, admission limits, and quota classification remain
unchanged. Raw upstream failure payloads remain available for forwarding.

## Verification

- Pre-fix red regression: one second was returned instead of five seconds.
- Post-fix runtime-proxy suite: 408 passed, none failed or ignored.
- Full workspace/all-target/all-feature Clippy with warnings denied: passed.
- Focused release regression suite before the final clock-codec assertion: 560
  passed across proxy, launch, Codex config, and optional tools.
- Static guards, including no-fallback, authority, ownership, size, and runtime
  test manifest: passed. No thresholds or assertions were weakened.
- Additional coverage includes nested versus outer advice, reverse-order duplicate
  header casing, CR/LF rejection, tabs, non-ASCII values, numeric JSON values,
  zero, dates in the past and future, invalid advice, the local cap, and unchanged
  quota/success classifications.

No live model request or credential-based upstream probe was used for these tests.
