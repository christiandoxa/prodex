# Smart Context rollout bucket ownership migrated to Mojo

The production rollout picker now uses the versioned compiled Mojo
rollout bucket in the existing Smart Context policy kernel. The host computes
SHA-256 from the stable key, but Mojo owns big-endian interpretation of the
first eight bytes and modulo 10,000 for the canary bucket. The Rust bucket
algorithm was deleted; there is no semantic Rust fallback.

Required-Mojo direct tests validate SHA-256 digest length, ABI version,
null pointers and big-endian numeric boundaries. Real runtime-proxy
regressions preserve five historical key-to-bucket assignments, the
20,000-key sample distribution, canary opt-out, explicit exact mode,
and shadow behavior. This change does not modify stored fingerprints,
hard affinity or existing session routing.

The long-term broad production Mojo share target remains 75% and is not
asserted complete at this checkpoint.
