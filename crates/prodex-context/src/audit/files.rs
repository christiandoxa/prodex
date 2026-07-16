use std::collections::HashSet;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::{is_compressible_context_file, is_context_backup};

pub(super) const CONTEXT_AUDIT_ROOTS: &[&str] = &[
    "AGENTS.md",
    "AGENTS.override.md",
    "memories",
    "memories_extensions",
    "rules",
    "skills",
];

const CONTEXT_WALK_MAX_DEPTH: usize = 64;
const CONTEXT_WALK_MAX_FILES: usize = 16_384;
pub(super) const CONTEXT_WALK_MAX_BYTES: u64 = 256 * 1024 * 1024;
const CONTEXT_AUDIT_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) struct ContextReadRoot {
    directory: VerifiedContextDirectory,
}

pub(super) struct ContextFileText {
    pub(super) text: String,
    pub(super) metadata: Metadata,
}

impl ContextReadRoot {
    pub(super) fn open(path: &Path) -> Result<Option<Self>> {
        Ok(VerifiedContextDirectory::open(path)?.map(|directory| Self { directory }))
    }

    pub(super) fn validate(&self) -> Result<()> {
        self.directory.validate()
    }
}

struct VerifiedContextDirectory {
    source: PathBuf,
    canonical: PathBuf,
    metadata: Metadata,
}

impl VerifiedContextDirectory {
    fn open(path: &Path) -> Result<Option<Self>> {
        let named = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to inspect context directory {}", path.display())
                });
            }
        };
        if named.file_type().is_symlink() || !named.is_dir() {
            return Ok(None);
        }

        let metadata = open_context_directory_metadata_no_follow(path)
            .with_context(|| format!("failed to open context directory {}", path.display()))?;
        let canonical = fs::canonicalize(path)
            .with_context(|| format!("failed to resolve context directory {}", path.display()))?;
        let canonical_metadata = fs::metadata(&canonical).with_context(|| {
            format!(
                "failed to inspect context directory {}",
                canonical.display()
            )
        })?;
        if !same_context_file_identity(&named, &metadata)
            || !same_context_file_identity(&metadata, &canonical_metadata)
        {
            bail!("context directory changed while opening {}", path.display());
        }
        Ok(Some(Self {
            source: path.to_path_buf(),
            canonical,
            metadata,
        }))
    }

    fn validate(&self) -> Result<()> {
        let named = fs::symlink_metadata(&self.source).with_context(|| {
            format!(
                "failed to recheck context directory {}",
                self.source.display()
            )
        })?;
        if named.file_type().is_symlink()
            || !same_context_file_identity(&self.metadata, &named)
            || fs::canonicalize(&self.source).ok().as_deref() != Some(self.canonical.as_path())
        {
            bail!(
                "context directory changed during traversal {}",
                self.source.display()
            );
        }
        Ok(())
    }
}

pub(crate) fn collect_context_files(path: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    let named = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect context path {}", path.display()));
        }
    };
    if named.file_type().is_symlink() {
        return Ok(());
    }
    if named.is_file() {
        let (_, metadata, _) = open_context_file_no_follow(path)?;
        enforce_context_walk_limits(1, metadata.len(), path)?;
        paths.push(path.to_path_buf());
        return Ok(());
    }
    let Some(root) = VerifiedContextDirectory::open(path)? else {
        return Ok(());
    };

    let mut pending = vec![(root.canonical.clone(), 0_usize)];
    let mut visited_directories = HashSet::new();
    let mut collected_files = 0_usize;
    let mut collected_bytes = 0_u64;
    while let Some((current, depth)) = pending.pop() {
        root.validate()?;
        let named = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to inspect context path {}", current.display())
                });
            }
        };
        if named.file_type().is_symlink() {
            continue;
        }
        if named.is_file() {
            let (_, metadata, canonical) = open_context_file_no_follow(&current)?;
            if !canonical.starts_with(&root.canonical) {
                bail!("context file escaped traversal root {}", current.display());
            }
            collected_files = collected_files.saturating_add(1);
            collected_bytes = collected_bytes.saturating_add(metadata.len());
            enforce_context_walk_limits(collected_files, collected_bytes, &current)?;
            paths.push(current);
            continue;
        }
        if !named.is_dir() {
            continue;
        }
        if depth >= CONTEXT_WALK_MAX_DEPTH {
            bail!("context traversal depth exceeded at {}", current.display());
        }
        let Some(directory) = VerifiedContextDirectory::open(&current)? else {
            continue;
        };
        if !directory.canonical.starts_with(&root.canonical) {
            bail!(
                "context directory escaped traversal root {}",
                current.display()
            );
        }
        if !visited_directories.insert(directory.canonical.clone()) {
            continue;
        }
        directory.validate()?;
        let mut entries = fs::read_dir(&directory.canonical)
            .with_context(|| {
                format!(
                    "failed to read context directory {}",
                    directory.canonical.display()
                )
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| {
                format!("failed to read entry in {}", directory.canonical.display())
            })?;
        directory.validate()?;
        root.validate()?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries.into_iter().rev() {
            pending.push((entry.path(), depth + 1));
        }
    }
    Ok(())
}

pub(super) fn read_context_file_bounded(
    root: &ContextReadRoot,
    path: &Path,
) -> Result<ContextFileText> {
    root.validate()?;
    let (mut file, metadata, canonical) = open_context_file_no_follow(path)?;
    if !canonical.starts_with(&root.directory.canonical) {
        bail!("context file escaped audit root {}", path.display());
    }
    let (text, metadata) =
        read_opened_context_file_bounded(&mut file, &metadata, CONTEXT_AUDIT_MAX_FILE_BYTES)
            .with_context(|| format!("failed to read context file {}", path.display()))?;
    root.validate()?;
    Ok(ContextFileText { text, metadata })
}

fn enforce_context_walk_limits(files: usize, bytes: u64, path: &Path) -> Result<()> {
    if files > CONTEXT_WALK_MAX_FILES || bytes > CONTEXT_WALK_MAX_BYTES {
        bail!("context traversal limit exceeded at {}", path.display());
    }
    Ok(())
}

fn open_context_file_no_follow(path: &Path) -> Result<(File, Metadata, PathBuf)> {
    let named = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect context file {}", path.display()))?;
    if named.file_type().is_symlink() || !named.is_file() {
        bail!("context path is not a regular file {}", path.display());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .with_context(|| format!("failed to open context file {}", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("failed to inspect context file {}", path.display()))?;
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("failed to resolve context file {}", path.display()))?;
    let canonical_metadata = fs::metadata(&canonical)
        .with_context(|| format!("failed to inspect context file {}", canonical.display()))?;
    if !same_context_file_identity(&named, &metadata)
        || !same_context_file_identity(&metadata, &canonical_metadata)
    {
        bail!("context file changed while opening {}", path.display());
    }
    Ok((file, metadata, canonical))
}

#[cfg(unix)]
fn open_context_directory_metadata_no_follow(path: &Path) -> io::Result<Metadata> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)?
        .metadata()
}

#[cfg(not(unix))]
fn open_context_directory_metadata_no_follow(path: &Path) -> io::Result<Metadata> {
    fs::metadata(path)
}

fn read_opened_context_file_bounded(
    file: &mut File,
    opened_metadata: &Metadata,
    limit: u64,
) -> io::Result<(String, Metadata)> {
    if !opened_metadata.is_file() || opened_metadata.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "context file is not regular or exceeds audit size limit",
        ));
    }
    let mut bytes = Vec::with_capacity(opened_metadata.len().min(limit) as usize);
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    let metadata = file.metadata()?;
    if bytes.len() as u64 > limit || metadata.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "context file exceeds audit size limit",
        ));
    }
    let text = String::from_utf8(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok((text, metadata))
}

#[cfg(unix)]
fn same_context_file_identity(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.file_type() == right.file_type() && left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_context_file_identity(left: &Metadata, right: &Metadata) -> bool {
    left.file_type() == right.file_type()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

pub(super) fn is_auditable_context_file(path: &Path) -> bool {
    !is_context_backup(path)
        && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
        && (is_compressible_context_file(path)
            || matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("toml" | "json" | "yaml" | "yml")
            ))
}

pub(super) fn is_static_duplicate_context_file(path: &Path) -> bool {
    is_compressible_context_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn bounded_reader_rejects_growth_after_opened_metadata() {
        let path = std::env::temp_dir().join(format!(
            "prodex-context-audit-growth-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        fs::write(&path, b"small").expect("seed context");
        let mut file = File::open(&path).expect("open context");
        let opened_metadata = file.metadata().expect("opened metadata");
        OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open appender")
            .write_all(b" grows past bound")
            .expect("grow context");

        let error = read_opened_context_file_bounded(&mut file, &opened_metadata, 8)
            .expect_err("growth must be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        let _ = fs::remove_file(path);
    }
}
