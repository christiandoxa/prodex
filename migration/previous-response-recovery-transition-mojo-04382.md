# Previous-response recovery transition state moved to compiled Mojo

The production runtime previous-response recovery orchestrator now delegates
its final precedence and transition flags to the compiled Mojo Responses
attempt owner. It first consumes the existing Mojo previous-response decision,
then uses a versioned typed ABI to select owner retry, strict stale-continuation
rejection, or rotation with explicit response/verified-affinity clearance.

The old Rust decision tree and duplicated retry-index/reset/affinity flag
selection were deleted. Rust still carries the parsed request and decision
facts, converts typed host enums, and applies the returned transition to the
existing retry and state-binding host. There is no alternate Rust semantic
recovery transition or silent fallback.

The Mojo transition has the following precise priorities:
- A permitted owner retry precedes every other outcome and retains all affinity.
- Strict stale-continuation policy blocks rotation and preserves ownership.
- Otherwise rotation resets retry state, releases response-profile affinity,
  and clears trusted affinity only if the caller's established policy permits.

The ABI validates version, every boolean flag, non-null and sufficient output
storage, plus retry-index overflow before touching output. Direct Mojo tests
cover all three outcomes and failure shapes. All 414 runtime-proxy library
tests and 47 WebSocket message caller tests pass. A no-fallback guard requires
the compiled transition and refuses restoration of the Rust decision branches.

This checkpoint is production-reachable, and the separate 75% Mojo production
share target remains unfinished.
