use std::io;
use std::path::Path;

use aes_gcm_siv::aead::{Aead, KeyInit, Payload};
use aes_gcm_siv::{Aes256GcmSiv, Nonce};
use zeroize::Zeroizing;

use crate::secure_file::{self, FileSecurity};

const PRIVATE_PAYLOAD_KEY_BYTES: usize = 32;
const PRIVATE_PAYLOAD_NONCE_BYTES: usize = 12;

/// Temporarily bypasses private-file ownership, permission, and ACL validation.
///
/// Path containment, symlink/reparse-point checks, regular-file checks, and size
/// limits remain active. Keep this guard scoped to a trusted profile operation.
pub struct InsecureFileAccessGuard {
    previous: bool,
}

/// Enables insecure private-file access until the returned guard is dropped.
pub fn allow_insecure_file_access() -> InsecureFileAccessGuard {
    InsecureFileAccessGuard {
        previous: secure_file::set_insecure_file_access(true),
    }
}

impl Drop for InsecureFileAccessGuard {
    fn drop(&mut self) {
        secure_file::set_insecure_file_access(self.previous);
    }
}

/// Creates or tightens a directory for current-user-private secret storage.
pub fn ensure_private_directory(path: &Path) -> io::Result<()> {
    secure_file::ensure_private_directory(path)
}

/// Reads a current-user-private regular file without following path indirection.
///
/// Returns `None` when the file does not exist and rejects untrusted parents,
/// unsafe ownership or permissions, and content larger than `max_bytes`.
pub fn read_private_file_bounded(
    path: &Path,
    max_bytes: u64,
) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
    secure_file::open_file(path, FileSecurity::Private)?
        .map(|file| file.read_bounded(max_bytes).map(Zeroizing::new))
        .transpose()
}

/// Atomically replaces `path` with a flushed current-user-private regular file.
pub fn write_private_file_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    secure_file::write_private_atomic(path, bytes)
}

/// Creates and flushes a current-user-private regular file without replacing an existing entry.
pub fn write_private_file_create_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    secure_file::create_private(path, bytes).map(drop)
}

/// Encrypts a private payload with a random nonce prepended to the ciphertext.
pub fn encrypt_private_payload(
    key: &[u8],
    associated_data: &[u8],
    plaintext: &[u8],
) -> io::Result<Zeroizing<Vec<u8>>> {
    if key.len() != PRIVATE_PAYLOAD_KEY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid private payload key length",
        ));
    }
    let cipher = Aes256GcmSiv::new_from_slice(key).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "failed to initialize private payload cipher",
        )
    })?;
    let mut nonce = [0_u8; PRIVATE_PAYLOAD_NONCE_BYTES];
    getrandom::fill(&mut nonce)
        .map_err(|_| io::Error::other("failed to generate private payload nonce"))?;
    let nonce_ref = <&Nonce>::try_from(nonce.as_slice()).expect("nonce length is fixed");
    let ciphertext = cipher
        .encrypt(
            nonce_ref,
            Payload {
                msg: plaintext,
                aad: associated_data,
            },
        )
        .map_err(|_| io::Error::other("failed to encrypt private payload"))?;
    let mut encoded = Zeroizing::new(Vec::with_capacity(nonce.len() + ciphertext.len()));
    encoded.extend_from_slice(&nonce);
    encoded.extend_from_slice(&ciphertext);
    Ok(encoded)
}

/// Decrypts a nonce-prefixed private payload and authenticates its associated data.
pub fn decrypt_private_payload(
    key: &[u8],
    associated_data: &[u8],
    encoded: &[u8],
) -> io::Result<Zeroizing<Vec<u8>>> {
    if key.len() != PRIVATE_PAYLOAD_KEY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid private payload key length",
        ));
    }
    if encoded.len() <= PRIVATE_PAYLOAD_NONCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated private payload",
        ));
    }
    let cipher = Aes256GcmSiv::new_from_slice(key).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "failed to initialize private payload cipher",
        )
    })?;
    let nonce = <&Nonce>::try_from(&encoded[..PRIVATE_PAYLOAD_NONCE_BYTES])
        .expect("nonce slice length is fixed");
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &encoded[PRIVATE_PAYLOAD_NONCE_BYTES..],
                aad: associated_data,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid private payload"))
}
