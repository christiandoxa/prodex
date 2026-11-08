//! Atomic metadata-prefix replacement with memory independent of rollout length.
//! The caller must hold native publication/writer guards through file and index repair.
use super::*;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::time::SystemTime;

const COPY_BUFFER_BYTES: usize = 128 * 1024;

pub(crate) struct SessionHeaderRepair {
    path: PathBuf,
    source: File,
    revision: SourceRevision,
    prefix: String,
    compressed: bool,
    modified: SystemTime,
    _lock: File,
}

impl SessionHeaderRepair {
    pub(crate) fn try_begin(
        root: &Path,
        path: &Path,
        max_header_bytes: u64,
    ) -> Result<Option<Self>> {
        let Some(lock) = try_repository_lock(root)? else {
            return Ok(None);
        };
        require_directory(path.parent().context("session has no parent")?)?;
        let (source, metadata) = open_regular_file(path, false)?;
        let revision = SourceRevision::from_metadata(&metadata);
        let modified = metadata.modified()?;
        let compressed = crate::session_file::is_compressed_session_file(path);
        let input: Box<dyn Read + '_> = if compressed {
            Box::new(zstd::stream::read::Decoder::new(&source)?)
        } else {
            Box::new(&source)
        };
        let mut reader = BufReader::new(input);
        let mut prefix = String::new();
        loop {
            let previous = prefix.len();
            let remaining = max_header_bytes.saturating_sub(previous as u64);
            let read = (&mut reader).take(remaining + 1).read_line(&mut prefix)?;
            anyhow::ensure!(
                prefix.len() as u64 <= max_header_bytes,
                "session header exceeds safe size limit"
            );
            anyhow::ensure!(read > 0, "missing session metadata");
            if !prefix[previous..].trim().is_empty() {
                break;
            }
        }
        drop(reader);
        verify_source(path, &source, &revision)?;
        Ok(Some(Self {
            path: path.into(),
            source,
            revision,
            prefix,
            compressed,
            modified,
            _lock: lock,
        }))
    }

    pub(crate) fn prefix(&self) -> &str {
        &self.prefix
    }

    pub(crate) fn commit(self, repaired_prefix: &[u8]) -> Result<()> {
        self.commit_inner(repaired_prefix, || {})
    }

    #[cfg(test)]
    pub(crate) fn commit_after_prepare(self, prefix: &[u8], action: impl FnOnce()) -> Result<()> {
        self.commit_inner(prefix, action)
    }

    fn commit_inner(mut self, repaired_prefix: &[u8], before_publish: impl FnOnce()) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("session has no parent")?
            .to_path_buf();
        verify_source(&self.path, &self.source, &self.revision)?;
        let mut backup = self.ensure_streamed_backup()?;
        let mut temporary = create_temporary_file(&self.path)?;
        if self.compressed {
            self.source.seek(SeekFrom::Start(0))?;
            let mut decoder = zstd::stream::read::Decoder::new(&self.source)?;
            let skipped = io::copy(
                &mut (&mut decoder).take(self.prefix.len() as u64),
                &mut io::sink(),
            )?;
            anyhow::ensure!(
                skipped == self.prefix.len() as u64,
                "compressed header changed during repair"
            );
            let mut encoder = zstd::stream::write::Encoder::new(temporary.file_mut(), 3)?;
            encoder.write_all(repaired_prefix)?;
            io::copy(&mut decoder, &mut encoder)?;
            encoder.finish()?;
        } else {
            temporary.file_mut().write_all(repaired_prefix)?;
            self.source
                .seek(SeekFrom::Start(self.prefix.len() as u64))?;
            let remainder = self.revision.length - self.prefix.len() as u64;
            let copied = io::copy(
                &mut (&mut self.source).take(remainder),
                temporary.file_mut(),
            )?;
            anyhow::ensure!(
                copied == remainder,
                "session shortened during header repair"
            );
        }
        // Repair must not make old chats appear more recent in the native picker.
        temporary.file_mut().set_modified(self.modified)?;
        temporary.file_mut().sync_all()?;
        sync_directory(&parent)?;
        before_publish();
        verify_source(&self.path, &self.source, &self.revision)?;
        drop(self.source);
        temporary.close();
        fs::rename(temporary.path(), &self.path)
            .context("failed to publish session header repair")?;
        temporary.disarm();
        if let Some(backup) = backup.as_mut() {
            backup.disarm();
        }
        temporary.verify_replaced(&self.path)?;
        sync_directory(&parent)
    }

    fn ensure_streamed_backup(&mut self) -> Result<Option<CreatedPrivateFile>> {
        let backup_path = self.path.with_extension(format!(
            "{}.prodex-provider-identity-bak",
            self.path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("session")
        ));
        self.source.seek(SeekFrom::Start(0))?;
        let created = match fs::symlink_metadata(&backup_path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // Publish only a complete durable snapshot. A terminated copy may leave
                // an unreferenced staging file, never a partial authoritative backup.
                let mut staging = create_temporary_file(&backup_path)?;
                let copied = io::copy(
                    &mut (&mut self.source).take(self.revision.length),
                    staging.file_mut(),
                )?;
                anyhow::ensure!(
                    copied == self.revision.length,
                    "session shortened during backup"
                );
                staging.file_mut().sync_all()?;
                verify_source(&self.path, &self.source, &self.revision)?;
                verify_named_file(staging.path(), staging.file())?;
                let file = staging.file().try_clone()?;
                let identity = Handle::from_file(file.try_clone()?)?;
                // Atomic create-if-absent: unlike rename, never overwrite an existing
                // recovery snapshot or a raced-in symlink on any supported platform.
                fs::hard_link(staging.path(), &backup_path)
                    .context("failed to publish provider backup")?;
                let backup = CreatedPrivateFile {
                    path: backup_path.clone(),
                    file: Some(file),
                    identity,
                    armed: true,
                };
                verify_named_file(&backup_path, backup.file())?;
                drop(staging);
                Some(backup)
            }
            Ok(_) => {
                let (mut backup, metadata) = open_regular_file(&backup_path, cfg!(windows))?;
                let revision = SourceRevision::from_metadata(&metadata);
                anyhow::ensure!(
                    metadata.len() == self.revision.length,
                    "provider backup differs from current session"
                );
                // Comparing incrementally keeps retry-after-interruption safe for multi-GiB files.
                let mut original = vec![0; COPY_BUFFER_BYTES];
                let mut retained = vec![0; COPY_BUFFER_BYTES];
                let mut remaining = self.revision.length;
                while remaining > 0 {
                    let count = remaining.min(COPY_BUFFER_BYTES as u64) as usize;
                    self.source.read_exact(&mut original[..count])?;
                    backup.read_exact(&mut retained[..count])?;
                    anyhow::ensure!(
                        original[..count] == retained[..count],
                        "provider backup differs from current session"
                    );
                    remaining -= count as u64;
                }
                verify_source(&backup_path, &backup, &revision)?;
                set_private_mode(&backup, &backup_path)?;
                backup.sync_all()?;
                verify_named_file(&backup_path, &backup)?;
                None
            }
            Err(error) => return Err(error).context("failed to create provider backup"),
        };
        verify_source(&self.path, &self.source, &self.revision)?;
        sync_directory(self.path.parent().context("session has no parent")?)?;
        Ok(created)
    }
}
