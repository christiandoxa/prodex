# WebSocket invalid previous-response ID code insertion migrated to Mojo

The compiled WebSocket failure domain now owns the eligibility rule for
inserting the previous_response_not_found code into an invalid response-ID
error. The rule requires all three facts: the existing Mojo error
classifier recognized an exact invalid ID, the nested JSON error object
exists, and the object does not already have a code field.

The prior Rust branches combining those facts were removed. Rust still
owns serde_json parsing, exact String/data ownership, and the concrete
mutation of the error object when the compiled Mojo plan allows it.
Previously written vendor codes are never overridden; a non-JSON or
binary frame remains unchanged. No duplicate Rust policy or silent
fallback has been retained.

Required-Mojo direct ABI tests cover all eight combinations of the
three input flags, unknown ABI versions, and non-Boolean fields.
Production regressions protect exact-ID recovery, near matches,
existing vendor codes, invalid JSON and raw binary payloads. The
committed-response no-replay and session-affinity invariants remain
unchanged. This is an incremental checkpoint, not completion of the
separate 75% Mojo production share project target.
