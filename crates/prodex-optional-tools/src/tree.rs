use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

const TOOL_MANIFEST: &str = "prodex-tool.json";
const MAX_TREE_FILES: usize = 4_096;
const MAX_TREE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

pub(crate) fn tree_sha256(root: &Path, digest_domain: &[u8]) -> Result<String> {
    let mut files = Vec::new();
    collect_tree_files(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let total_bytes = files
        .iter()
        .try_fold(0_u64, |total, (_, _, len)| total.checked_add(*len))
        .context("optional-tool tree size overflow")?;
    ensure!(
        total_bytes <= MAX_TREE_BYTES,
        "optional-tool tree is too large"
    );

    let mut hasher = Sha256::new();
    hasher.update(digest_domain);
    for (relative, path, expected_len) in files {
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(expected_len.to_be_bytes());
        let bytes = read_bounded_file(&path, MAX_FILE_BYTES)?;
        ensure!(
            bytes.len() as u64 == expected_len,
            "{} changed while it was validated",
            path.display()
        );
        hasher.update(bytes);
    }
    Ok(crate::optional_tools::hex_digest(&hasher.finalize()))
}

pub(crate) fn read_bounded_file(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= limit,
        "{} is not a bounded regular file",
        path.display()
    );
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(path)
        .with_context(|| format!("failed to read {}", path.display()))?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    ensure!(
        bytes.len() as u64 <= limit,
        "{} is too large",
        path.display()
    );
    Ok(bytes)
}

fn collect_tree_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf, u64)>,
) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", directory.display()))?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .context("optional-tool tree path escaped its root")?;
        if relative == Path::new(".git") {
            continue;
        }
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        ensure!(
            !file_type.is_symlink(),
            "optional-tool tree contains symlink {}",
            path.display()
        );
        if file_type.is_dir() {
            collect_tree_files(root, &path, files)?;
            continue;
        }
        ensure!(
            file_type.is_file(),
            "optional-tool tree contains unsupported file {}",
            path.display()
        );
        if relative == Path::new(TOOL_MANIFEST) {
            continue;
        }
        ensure!(
            files.len() < MAX_TREE_FILES,
            "optional-tool tree has too many files"
        );
        let relative = normalized_relative_path(relative)?;
        let len = entry
            .metadata()
            .with_context(|| format!("failed to inspect {}", path.display()))?
            .len();
        ensure!(
            len <= MAX_FILE_BYTES,
            "optional-tool file {} is too large",
            path.display()
        );
        files.push((relative, path, len));
    }
    Ok(())
}

fn normalized_relative_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            bail!("invalid optional-tool tree path {}", path.display());
        };
        parts.push(
            part.to_str()
                .with_context(|| format!("non-UTF-8 optional-tool path {}", path.display()))?,
        );
    }
    Ok(parts.join("/"))
}
