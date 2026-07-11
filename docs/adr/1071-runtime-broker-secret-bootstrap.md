# ADR 1071: Runtime Broker Secret Bootstrap

## Status

Accepted

## Context

The runtime broker was launched with its instance and admin tokens in hidden CLI
arguments. Those values were observable through process listings and command-plan
debug output. The registry then persisted the admin token in both its primary JSON
file and last-good backup, and the health response repeated the instance token.

The broker still needs a per-process admin capability for authenticated loopback
health, metrics, and activation requests. Startup, readiness polling, compatible
broker reuse, and cross-platform process launch must remain unchanged.

## Decision

Launch the hidden `__runtime-broker` child with no configuration arguments. The
parent writes one versioned JSON bootstrap document to the child's piped standard
input and closes the pipe. The document is limited to 64 KiB, rejects unknown or
missing fields, validates bounded public fields, and rejects malformed, truncated,
oversized, or unsupported-version input before starting the proxy.

Admin capabilities use `RuntimeBrokerSecret`, which has redacted `Debug`, no
`Display` or value `Clone`, and zeroizes its owned bytes on drop. Shared broker
metadata clones only an `Arc` to that wrapper. Bootstrap serialization and input
buffers are also zeroized. All broker admin authentication uses the wrapper's one
central constant-time comparison.

Registry JSON contains only reusable public metadata and a random non-secret
`instance_id`. Health reports that identifier, never the admin capability. The raw
capability is stored once in a versioned envelope at
`runtime-broker-<key>.capability`:

- it is a regular, non-symlink file bounded to 8 KiB, with the secret itself
  bounded to 4 KiB;
- Unix creation mode is `0600`, and reads reject group or other permissions;
- it has no last-good backup;
- its public `instance_id` binds the secret to one broker generation, so an old
  broker cannot delete or use a newly rotated capability;
- registry metadata is published before the capability, and readiness succeeds
  only after the matching envelope can authenticate health;
- rotation replaces the single final file directly, and matching-instance
  cleanup removes it together with registry metadata; and
- removal best-effort overwrites a small regular file before unlinking it.

The control client loads the capability only when making a loopback admin request.
Legacy registries containing raw `instance_token` and `admin_token` fields are not
reused: Prodex stops that legacy broker and removes both registry copies before
starting the new protocol.

## Consequences

- Broker secrets no longer appear in argv, environment configuration, command
  plans, registry JSON, registry backups, health responses, or derived debug text.
- A user able to read the private capability file can still administer that local
  broker; filesystem permissions remain part of the trust boundary.
- Existing live brokers using the legacy registry schema restart once during
  migration instead of being reused with exposed credentials.
- Bootstrap protocol changes require a version increment and an explicit
  compatibility decision.
- On Windows, capability files inherit the user-scoped `PRODEX_HOME` directory
  ACL and use file-identity checks across open/remove races. Explicit DACL
  normalization remains part of the broader filesystem-hardening phase.

## Verification

```bash
cargo test --locked -p prodex-runtime-broker
cargo test --locked -p prodex-app runtime_broker_ -- --test-threads=1
cargo clippy --locked -p prodex-runtime-broker -p prodex-app --all-targets -- -D warnings
```
