# Ping status labels and temporary classification in Mojo

The ping diagnostics path now sends human labels, JSON labels, and temporary-failure
classification through the compiled `prodex_mojo_ping_status_label_v1` and
`prodex_mojo_ping_status_temporary_v1` operations. Rust keeps the typed status enum,
ABI mapping, and output materialization. No Rust label or temporary-status decision
fallback remains.

Required-Mojo coverage includes every status label, quota exclusion from temporary
failures, invalid ABI inputs, and the production `prodex-app` ping caller tests.
