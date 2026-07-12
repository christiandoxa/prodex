# ADR 1073: Profile Export V2 with Argon2id

## Status

Accepted

## Context

Encrypted profile exports used one version-1 PBKDF2-SHA256 envelope with an
exact compiled iteration count. Import therefore rejected otherwise safe
version-1 parameter values, while an attacker-controlled count needed to remain
bounded before password lookup. Passwords, derived keys, and decrypted JSON also
used ordinary buffers whose drop behavior did not erase their contents.

Version-1 files must remain importable. A stronger default must use a new
envelope version so existing ciphertext meaning never changes.

## Decision

Plain exports remain `payload_kind: "plain"`, version 1. New protected exports
use `payload_kind: "encrypted_v2"`, version 2, AES-256-GCM-SIV, and a nested KDF
object:

```json
{
  "algorithm": "argon2id",
  "version": 19,
  "memory_kib": 65536,
  "iterations": 3,
  "parallelism": 1
}
```

Import continues to accept the original `payload_kind: "encrypted"`, version-1
PBKDF2-SHA256 shape. Version-1 iteration counts are accepted from 50,000 through
2,000,000; the historical writer value is 100,000. Version 2 accepts Argon2id
version 19 with these bounds:

- memory: 8,192 through 131,072 KiB;
- iterations: 1 through 6; and
- parallelism: 1 through 4.

The parser validates the version/KDF pairing, cipher, KDF bounds, base64 shape,
salt, nonce, and ciphertext size before requesting a password or deriving a key.
The following hard limits apply:

- envelope: 64 MiB;
- ciphertext: 32 MiB plus the 16-byte authentication tag;
- serialized or decrypted plaintext: 32 MiB;
- password: 4 KiB;
- each nested auth/secret JSON string: 2 MiB;
- profiles: 256; and
- secret files: 16 per profile and 4,096 total.

Encrypted serialization plaintext, password copies, derived keys, decoded
ciphertext, decrypted plaintext, salt, and nonce buffers use zeroize-on-drop
storage. Profile payload, secret-file, and envelope `Debug` implementations
redact plaintext and ciphertext fields. The public bounded parser is
`parse_profile_export_envelope<T>(&[u8])` and is also the import and fuzz entry
point. The unused raw-key `derive_profile_export_key` compatibility helper was removed: returning
a plain array made the library unable to guarantee derived-key erasure. Version-1 envelope
decryption compatibility remains in the bounded import path and therefore does not require a
public raw-key API.

## Argon2id Parameter Measurement

The benchmark ran on 2026-07-11 with Rust 1.97 in release mode on an AMD Ryzen 5
PRO 4650G (6 cores/12 threads) with 30 GiB RAM. Each row is five sequential key
derivations; no application-performance claim is inferred from this KDF-only
measurement.

```bash
cargo run --locked --release -p prodex-profile-export --example argon2id_benchmark -- 32768 3 1 5
cargo run --locked --release -p prodex-profile-export --example argon2id_benchmark -- 65536 3 1 5
cargo run --locked --release -p prodex-profile-export --example argon2id_benchmark -- 131072 3 1 5
```

| Memory | Min | Median | Mean | Sample SD | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| 32 MiB | 63.351 ms | 64.683 ms | 64.726 ms | 1.074 ms | 66.334 ms |
| 64 MiB | 127.648 ms | 129.156 ms | 130.267 ms | 2.566 ms | 134.169 ms |
| 128 MiB | 259.715 ms | 263.782 ms | 263.148 ms | 2.577 ms | 266.226 ms |

The 64 MiB result is the default: it keeps an interactive export/import near
130 ms per derivation on this host while doubling the measured memory cost of
the 32 MiB candidate. The accepted range permits deliberate future tuning
without redefining version 2.

## Consequences

- CLI flags, password prompts, and import commands do not change.
- Existing version-1 encrypted exports remain decryptable; a checked-in
  deterministic version-1 fixture guards that contract.
- Newly encrypted files are not readable by Prodex releases that only understand
  version 1. Users needing an old release must export without protection or use
  the old release to create its own version-1 encrypted file.
- Malformed, corrupt, wrong-password, and excessive-resource inputs fail with
  bounded, redacted errors before profile import side effects.
- Bundle reads and writes use the shared private-file boundary: trusted parent
  directories, no-follow/reparse-point opens, owner and Unix mode or Windows
  DACL validation, bounded zeroize-on-drop reads, and atomic private replacement
  with flush/write-through and identity verification. A final symlink is safely
  replaced during export without writing through to its target.
- Import intentionally rejects existing bundles that are public, owned by a
  different user, or stored below an untrusted parent. Secure and own an existing
  bundle (including mode `0600` on Unix) before importing it, or re-export it.

## Verification

```bash
cargo test --locked -p prodex-profile-export -- --test-threads=1
cargo test --locked -p prodex-secret-store -- --test-threads=1
cargo test --locked -p prodex-app profile_export_ -- --test-threads=1
cargo clippy --locked -p prodex-secret-store -p prodex-profile-export -p prodex-app --all-targets -- -D warnings
```
