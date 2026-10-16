# SSE precommit retry and profile-rotation flags owned by Mojo

The compiled SSE kernel now owns the complete mapping between an upstream
HTTP error policy's class and action, and the terminal event flags used
by precommit SSE inspection. The Rust OR/AND policy for quota-blocked,
rate-limited and overloaded has been removed from production.

Mojo maps RotateProfile + Quota to quota-blocked, RetryProfile + RateLimited
to rate-limited, RetryProfile + Overload/TransientServer to overloaded,
and RotateProfile + ProfileUnavailable to overloaded. PassThrough actions
always generate no retry flags, protecting already-committed response state.
Rust retains the type-safe enum-to-ABI mapping and the serde_json error
payload parser but does not recalculate the Boolean decision.

Direct compiled Mojo ABI tests cover every one of the 18 possible
class/action combinations, invalid tags and version errors. Production
regressions use real response.failed JSON cases for quota, rate limits,
overload, deactivated workspace and unrelated error codes.

This is an incremental production-reachable checkpoint, not completion
of the separate 75% broad production Mojo share goal.
