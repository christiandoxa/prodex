use std::collections::HashSet;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::{is_compressible_context_file, is_context_backup};

use super::safe_path;

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
                    format!("failed to inspect context directory {}", safe_path(path))
                });
            }
        };
        if named.file_type().is_symlink() || !named.is_dir() {
            return Ok(None);
        }

        let metadata = open_context_directory_metadata_no_follow(path)
            .with_context(|| format!("failed to open context directory {}", safe_path(path)))?;
        let canonical = fs::canonicalize(path)
            .with_context(|| format!("failed to resolve context directory {}", safe_path(path)))?;
        let canonical_metadata = fs::metadata(&canonical).with_context(|| {
            format!(
                "failed to inspect context directory {}",
                safe_path(&canonical)
            )
        })?;
        if !same_context_file_identity(&named, &metadata)
            || !same_context_file_identity(&metadata, &canonical_metadata)
        {
            bail!(
                "context directory changed while opening {}",
                safe_path(path)
            );
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
                safe_path(&self.source)
            )
        })?;
        if named.file_type().is_symlink()
            || !same_context_file_identity(&self.metadata, &named)
            || fs::canonicalize(&self.source).ok().as_deref() != Some(self.canonical.as_path())
        {
            bail!(
                "context directory changed during traversal {}",
                safe_path(&self.source)
            );
        }
        Ok(())
    }
}

pub(crate) fn collect_context_files(path: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    collect_context_files_inner(path, paths, false, &mut |_| {})
}

pub(super) fn collect_context_files_for_audit(
    path: &Path,
    paths: &mut Vec<PathBuf>,
    on_error: &mut dyn FnMut(&Path),
) -> Result<()> {
    collect_context_files_inner(path, paths, true, on_error)
}

fn collect_context_files_inner(
    path: &Path,
    paths: &mut Vec<PathBuf>,
    recover_errors: bool,
    on_error: &mut dyn FnMut(&Path),
) -> Result<()> {
    let Some(named) = recover_context_walk_operation(
        context_path_metadata(path),
        path,
        recover_errors,
        on_error,
    )?
    .flatten() else {
        return Ok(());
    };
    if named.file_type().is_symlink() {
        return Ok(());
    }
    if named.is_file() {
        let Some((_, metadata, _)) = recover_context_walk_operation(
            open_context_file_no_follow(path),
            path,
            recover_errors,
            on_error,
        )?
        else {
            return Ok(());
        };
        enforce_context_walk_limits(1, metadata.len(), path)?;
        paths.push(path.to_path_buf());
        return Ok(());
    }
    let Some(root) = recover_context_walk_operation(
        VerifiedContextDirectory::open(path),
        path,
        recover_errors,
        on_error,
    )?
    .flatten() else {
        return Ok(());
    };
    collect_context_directory_files(&root, paths, recover_errors, on_error)
}

fn collect_context_directory_files(
    root: &VerifiedContextDirectory,
    paths: &mut Vec<PathBuf>,
    recover_errors: bool,
    on_error: &mut dyn FnMut(&Path),
) -> Result<()> {
    let mut pending = vec![(root.canonical.clone(), 0_usize)];
    let mut visited_directories = HashSet::new();
    let mut collected_files = 0_usize;
    let mut collected_bytes = 0_u64;
    while let Some((current, depth)) = pending.pop() {
        root.validate()?;
        let Some(named) = recover_context_walk_operation(
            context_path_metadata(&current),
            &current,
            recover_errors,
            on_error,
        )?
        .flatten() else {
            continue;
        };
        if named.file_type().is_symlink() {
            continue;
        }
        if named.is_file() {
            collect_context_file_from_root(
                root,
                current,
                paths,
                &mut collected_files,
                &mut collected_bytes,
                recover_errors,
                on_error,
            )?;
            continue;
        }
        if !named.is_dir() {
            continue;
        }
        enqueue_context_directory(
            root,
            current,
            depth,
            &mut visited_directories,
            &mut pending,
            recover_errors,
            on_error,
        )?;
    }
    Ok(())
}

fn recover_context_walk_operation<T>(
    result: Result<T>,
    path: &Path,
    recover_errors: bool,
    on_error: &mut dyn FnMut(&Path),
) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(_) if recover_errors => {
            on_error(path);
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn context_path_metadata(path: &Path) -> Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("failed to inspect context path {}", safe_path(path))),
    }
}

fn collect_context_file_from_root(
    root: &VerifiedContextDirectory,
    current: PathBuf,
    paths: &mut Vec<PathBuf>,
    collected_files: &mut usize,
    collected_bytes: &mut u64,
    recover_errors: bool,
    on_error: &mut dyn FnMut(&Path),
) -> Result<()> {
    let Some((_, metadata, canonical)) = recover_context_walk_operation(
        open_context_file_no_follow(&current),
        &current,
        recover_errors,
        on_error,
    )?
    else {
        return Ok(());
    };
    if !canonical.starts_with(&root.canonical) {
        bail!(
            "context file escaped traversal root {}",
            safe_path(&current)
        );
    }
    *collected_files = collected_files.saturating_add(1);
    *collected_bytes = collected_bytes.saturating_add(metadata.len());
    enforce_context_walk_limits(*collected_files, *collected_bytes, &current)?;
    paths.push(current);
    Ok(())
}

fn enqueue_context_directory(
    root: &VerifiedContextDirectory,
    current: PathBuf,
    depth: usize,
    visited_directories: &mut HashSet<PathBuf>,
    pending: &mut Vec<(PathBuf, usize)>,
    recover_errors: bool,
    on_error: &mut dyn FnMut(&Path),
) -> Result<()> {
    if depth >= CONTEXT_WALK_MAX_DEPTH {
        bail!(
            "context traversal depth exceeded at {}",
            safe_path(&current)
        );
    }
    let Some(directory) = recover_context_walk_operation(
        VerifiedContextDirectory::open(&current),
        &current,
        recover_errors,
        on_error,
    )?
    .flatten() else {
        return Ok(());
    };
    if !directory.canonical.starts_with(&root.canonical) {
        bail!(
            "context directory escaped traversal root {}",
            safe_path(&current)
        );
    }
    if !visited_directories.insert(directory.canonical.clone()) {
        return Ok(());
    }
    if recover_context_walk_operation(directory.validate(), &current, recover_errors, on_error)?
        .is_none()
    {
        return Ok(());
    }
    let Some(read_dir) = recover_context_walk_operation(
        fs::read_dir(&directory.canonical).with_context(|| {
            format!(
                "failed to read context directory {}",
                safe_path(&directory.canonical)
            )
        }),
        &current,
        recover_errors,
        on_error,
    )?
    else {
        return Ok(());
    };
    let mut entries = Vec::new();
    for entry in read_dir {
        if let Some(entry) = recover_context_walk_operation(
            entry.with_context(|| {
                format!(
                    "failed to read entry in {}",
                    safe_path(&directory.canonical)
                )
            }),
            &current,
            recover_errors,
            on_error,
        )? {
            entries.push(entry);
        }
    }
    if recover_context_walk_operation(directory.validate(), &current, recover_errors, on_error)?
        .is_none()
    {
        return Ok(());
    }
    root.validate()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries.into_iter().rev() {
        pending.push((entry.path(), depth + 1));
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
        bail!("context file escaped audit root {}", safe_path(path));
    }
    let (text, metadata) =
        read_opened_context_file_bounded(&mut file, &metadata, CONTEXT_AUDIT_MAX_FILE_BYTES)
            .with_context(|| format!("failed to read context file {}", safe_path(path)))?;
    root.validate()?;
    Ok(ContextFileText { text, metadata })
}

fn enforce_context_walk_limits(files: usize, bytes: u64, path: &Path) -> Result<()> {
    if files > CONTEXT_WALK_MAX_FILES || bytes > CONTEXT_WALK_MAX_BYTES {
        bail!("context traversal limit exceeded at {}", safe_path(path));
    }
    Ok(())
}

fn open_context_file_no_follow(path: &Path) -> Result<(File, Metadata, PathBuf)> {
    let named = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect context file {}", safe_path(path)))?;
    if named.file_type().is_symlink() || !named.is_file() {
        bail!("context path is not a regular file {}", safe_path(path));
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
        .with_context(|| format!("failed to open context file {}", safe_path(path)))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("failed to inspect context file {}", safe_path(path)))?;
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("failed to resolve context file {}", safe_path(path)))?;
    let canonical_metadata = fs::metadata(&canonical)
        .with_context(|| format!("failed to inspect context file {}", safe_path(&canonical)))?;
    if !same_context_file_identity(&named, &metadata)
        || !same_context_file_identity(&metadata, &canonical_metadata)
    {
        bail!("context file changed while opening {}", safe_path(path));
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

pub(super) fn is_auditable_context_file(path: &Path) -> Result<bool> {
    if is_context_backup(path) {
        return Ok(false);
    }
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect context file {}", safe_path(path)))?;
    Ok(metadata.file_type().is_file()
        && (is_compressible_context_file(path)
            || matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("toml" | "json" | "yaml" | "yml")
            )))
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
