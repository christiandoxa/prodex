# Housekeeping stale-login retention in Mojo

The stale login-directory cutoff now runs through operation 7 of
`prodex_core_file_policy_v1`. Rust still acquires filesystem metadata, walks the
managed-profile directory, performs deletion, and reports cleanup failures; the
compiled Mojo policy owns only the deterministic `modified < oldest_allowed`
decision. ABI boundary cases and housekeeping caller tests cover stale, equal,
and fresh timestamps.
