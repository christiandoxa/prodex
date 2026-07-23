use std::io;
use std::path::Path;

use zeroize::Zeroizing;

use crate::secure_file::{self, FileSecurity};

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
