# Super-expose rollout filename policy in Mojo

Rollout filename classification and unique UUID-window planning now run through
the compiled `prodex_session_prompt_write_rollout_name_v1` policy. Rust retains
path extraction, canonicalization, symlink and open-file authority, UUID parsing,
and filesystem/process effects. ABI and Super-expose caller tests cover legacy,
compressed, malformed, and duplicate-UUID filenames.
