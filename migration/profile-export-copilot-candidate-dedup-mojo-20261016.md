# Copilot import candidate deduplication migrated to Mojo

Copilot profile import now delegates preferred-user ordering and exact
`(host, login)` candidate deduplication to the compiled
`prodex_profile_export_copilot_candidate_users_v1` operation. Rust retains
config parsing, credential lookup, bounded ABI adaptation, and candidate
materialization; no Rust candidate-ordering fallback remains.

Required-Mojo ABI and profile-export caller regressions cover last-user
preference, duplicate removal, empty state, invalid ABI, and capacity.
