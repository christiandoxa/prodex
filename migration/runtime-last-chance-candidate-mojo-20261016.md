# Runtime last-chance candidate policy

The quota last-chance fallback now asks the compiled `candidate_decision.mojo`
ABI to evaluate cached auth, probe, snapshot, auth-failure, fallback-skip, and
backoff gates. Rust still owns state projection, profile ordering, hard
in-flight admission, and route-circuit or transport effects. The ABI is
fail-closed and has caller coverage for accepted backoff kinds and rejection
boundaries.
