use super::*;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::sync::atomic::{AtomicU64, Ordering};

const CONTEXT_COMPRESS_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
static CONTEXT_COMPRESS_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn compress_context_path(path: &Path, dry_run: bool) -> Result<ContextCompressReport> {
    let mut paths = Vec::new();
    collect_context_files(path, &mut paths)?;
    paths.sort();
    paths.dedup();

    let mut entries = Vec::new();
    for path in paths {
        entries.push(compress_context_file(&path, dry_run)?);
    }
    Ok(ContextCompressReport { entries })
}

fn compress_context_file(path: &Path, dry_run: bool) -> Result<ContextCompressEntry> {
    compress_context_file_before_replace(path, dry_run, || {})
}

fn compress_context_file_before_replace(
    path: &Path,
    dry_run: bool,
    before_replace: impl FnOnce(),
) -> Result<ContextCompressEntry> {
    if !is_compressible_context_file(path) {
        return Ok(ContextCompressEntry {
            path: path.to_path_buf(),
            backup_path: None,
            status: "skipped_not_prose".to_string(),
            original_bytes: 0,
            compressed_bytes: 0,
            estimated_tokens_before: 0,
            estimated_tokens_after: 0,
        });
    }

    let (original, original_metadata) = read_context_file_no_follow(path)?;
    let compressed = compress_context_text(&original);
    let original_bytes = original.len() as u64;
    let compressed_bytes = compressed.len() as u64;
    let estimated_tokens_before = estimate_context_tokens(
        original.chars().count(),
        original.split_whitespace().count(),
    );
    let estimated_tokens_after = estimate_context_tokens(
        compressed.chars().count(),
        compressed.split_whitespace().count(),
    );
    let backup_path = context_backup_path(path);

    if backup_path.exists() {
        return Ok(ContextCompressEntry {
            path: path.to_path_buf(),
            backup_path: Some(backup_path),
            status: "skipped_backup_exists".to_string(),
            original_bytes,
            compressed_bytes,
            estimated_tokens_before,
            estimated_tokens_after,
        });
    }

    if compressed_bytes >= original_bytes {
        return Ok(ContextCompressEntry {
            path: path.to_path_buf(),
            backup_path: Some(backup_path),
            status: "skipped_no_gain".to_string(),
            original_bytes,
            compressed_bytes,
            estimated_tokens_before,
            estimated_tokens_after,
        });
    }

    if !dry_run {
        let backup_metadata = match write_context_backup_create_new(
            &backup_path,
            original.as_bytes(),
            original_metadata.permissions(),
        ) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Ok(ContextCompressEntry {
                    path: path.to_path_buf(),
                    backup_path: Some(backup_path),
                    status: "skipped_backup_exists".to_string(),
                    original_bytes,
                    compressed_bytes,
                    estimated_tokens_before,
                    estimated_tokens_after,
                });
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to write backup {}", backup_path.display()));
            }
        };
        before_replace();
        if let Err(replace_error) =
            replace_context_file_if_unchanged(path, compressed.as_bytes(), &original_metadata)
        {
            if let Err(cleanup_error) =
                remove_context_backup_if_owned(&backup_path, &backup_metadata)
            {
                return Err(cleanup_error).with_context(|| {
                    format!(
                        "failed to clean backup {} after replacement failed: {replace_error}",
                        backup_path.display()
                    )
                });
            }
            return Err(replace_error)
                .with_context(|| format!("failed to write compressed context {}", path.display()));
        }
        sync_context_parent(path)
            .with_context(|| format!("failed to sync compressed context {}", path.display()))?;
    }

    Ok(ContextCompressEntry {
        path: path.to_path_buf(),
        backup_path: Some(backup_path),
        status: if dry_run {
            "dry_run".to_string()
        } else {
            "compressed".to_string()
        },
        original_bytes,
        compressed_bytes,
        estimated_tokens_before,
        estimated_tokens_after,
    })
}

pub fn render_context_compress_report(report: &ContextCompressReport, dry_run: bool) -> String {
    let title = if dry_run {
        "Context Compress Dry Run"
    } else {
        "Context Compress"
    };
    let mut lines = vec![section_header(title)];
    if report.entries.is_empty() {
        lines.push("No files matched.".to_string());
        return lines.join("\n");
    }

    for entry in &report.entries {
        let saved = entry.original_bytes.saturating_sub(entry.compressed_bytes);
        let token_saved = entry
            .estimated_tokens_before
            .saturating_sub(entry.estimated_tokens_after);
        lines.push(format!(
            "{}: {} ({} bytes saved, ~{} tokens saved)",
            entry.status,
            entry.path.display(),
            format_count(saved),
            format_count(token_saved),
        ));
        if let Some(backup_path) = &entry.backup_path
            && entry.status == "compressed"
        {
            lines.push(format!("Backup: {}", backup_path.display()));
        }
    }
    lines.join("\n")
}

pub fn compress_context_text(input: &str) -> String {
    let mut output = Vec::new();
    let mut paragraph = Vec::new();
    let mut in_fence = false;
    let mut previous_blank = false;

    for line in input.lines() {
        let trimmed = line.trim();
        let fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if fence {
            flush_context_paragraph(&mut paragraph, &mut output);
            output.push(line.to_string());
            in_fence = !in_fence;
            previous_blank = false;
            continue;
        }

        if in_fence || protected_context_line(line) {
            flush_context_paragraph(&mut paragraph, &mut output);
            output.push(line.to_string());
            previous_blank = false;
            continue;
        }

        if trimmed.is_empty() {
            flush_context_paragraph(&mut paragraph, &mut output);
            if !previous_blank && !output.is_empty() {
                output.push(String::new());
            }
            previous_blank = true;
            continue;
        }

        paragraph.push(trimmed.to_string());
        previous_blank = false;
    }

    flush_context_paragraph(&mut paragraph, &mut output);
    while output.last().is_some_and(|line| line.is_empty()) {
        output.pop();
    }
    if output.is_empty() {
        String::new()
    } else {
        format!("{}\n", output.join("\n"))
    }
}

fn flush_context_paragraph(paragraph: &mut Vec<String>, output: &mut Vec<String>) {
    if paragraph.is_empty() {
        return;
    }
    let joined = paragraph.join(" ");
    output.push(compact_context_prose(&joined));
    paragraph.clear();
}

fn compact_context_prose(input: &str) -> String {
    let normalized = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.contains('`') || normalized.contains("://") {
        return normalized;
    }

    let mut text = format!(" {normalized} ");
    for (from, to) in [
        (" in order to ", " to "),
        (" due to the fact that ", " because "),
        (" at this point in time ", " now "),
        (" make sure to ", " ensure "),
        (" it is important to ", " "),
        (" please note that ", " "),
        (" you should ", " should "),
        (" you must ", " must "),
    ] {
        text = text.replace(from, to);
    }

    text.split_whitespace()
        .filter(|word| !is_context_filler_word(word))
        .collect::<Vec<_>>()
        .join(" ")
}

fn protected_context_line(line: &str) -> bool {
    let trimmed = line.trim();
    let indented = line.starts_with("    ") || line.starts_with('\t');
    trimmed.starts_with('#')
        || trimmed.starts_with('|')
        || trimmed.starts_with('>')
        || trimmed.starts_with('$')
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.contains('`')
        || trimmed.contains("://")
        || indented
}

fn is_context_filler_word(word: &str) -> bool {
    let normalized = word
        .trim_matches(|ch: char| !ch.is_alphanumeric())
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "very" | "really" | "actually" | "basically" | "simply" | "please" | "just"
    )
}

pub(crate) fn estimate_context_tokens(chars: usize, words: usize) -> usize {
    chars.div_ceil(4).max((words * 4).div_ceil(3))
}

pub(crate) fn is_compressible_context_file(path: &Path) -> bool {
    !is_context_backup(path)
        && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
        && matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("md" | "markdown" | "txt")
        )
}

pub(crate) fn is_context_backup(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".original.md"))
}

fn context_backup_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("context");
    parent.join(format!("{stem}.original.md"))
}

fn read_context_file_no_follow(path: &Path) -> Result<(String, Metadata)> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(not(unix))]
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "context file must not be a symlink",
        ))
        .with_context(|| format!("failed to read context file {}", path.display()));
    }
    let file = options
        .open(path)
        .with_context(|| format!("failed to open context file {}", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("failed to inspect context file {}", path.display()))?;
    if !metadata.is_file() || metadata.len() > CONTEXT_COMPRESS_MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "context file is not regular or exceeds size limit",
        ))
        .with_context(|| format!("failed to read context file {}", path.display()));
    }
    let mut original = String::new();
    file.take(CONTEXT_COMPRESS_MAX_FILE_BYTES.saturating_add(1))
        .read_to_string(&mut original)
        .with_context(|| format!("failed to read context file {}", path.display()))?;
    if original.len() as u64 > CONTEXT_COMPRESS_MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "context file exceeds size limit",
        ))
        .with_context(|| format!("failed to read context file {}", path.display()));
    }
    Ok((original, metadata))
}

fn write_context_backup_create_new(
    path: &Path,
    bytes: &[u8],
    permissions: fs::Permissions,
) -> io::Result<Metadata> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(permissions.mode());
    }
    let mut file = options.open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    if let Err(error) = file
        .set_permissions(permissions)
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    let metadata = file.metadata()?;
    if let Err(error) = sync_context_parent(path) {
        drop(file);
        let _ = fs::remove_file(path);
        let _ = sync_context_parent(path);
        return Err(error);
    }
    Ok(metadata)
}

fn remove_context_backup_if_owned(path: &Path, expected: &Metadata) -> io::Result<bool> {
    let current = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if current.file_type().is_symlink() || !same_context_file_version(expected, &current) {
        return Ok(false);
    }
    fs::remove_file(path)?;
    sync_context_parent(path)?;
    Ok(true)
}

fn replace_context_file_if_unchanged(
    path: &Path,
    bytes: &[u8],
    original_metadata: &Metadata,
) -> io::Result<()> {
    let current_metadata = fs::symlink_metadata(path)?;
    if !same_context_file_version(original_metadata, &current_metadata) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "context file changed during compression",
        ));
    }
    let (temp_path, mut temp_file) =
        create_context_temp_file(path, original_metadata.permissions())?;
    if let Err(error) = temp_file
        .write_all(bytes)
        .and_then(|()| temp_file.sync_all())
    {
        drop(temp_file);
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    if let Err(error) = temp_file
        .set_permissions(original_metadata.permissions())
        .and_then(|()| temp_file.sync_all())
    {
        drop(temp_file);
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    drop(temp_file);
    let current_metadata = fs::symlink_metadata(path)?;
    if !same_context_file_version(original_metadata, &current_metadata) {
        let _ = fs::remove_file(&temp_path);
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "context file changed during compression",
        ));
    }
    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    Ok(())
}

fn create_context_temp_file(
    path: &Path,
    permissions: fs::Permissions,
) -> io::Result<(PathBuf, File)> {
    for _ in 0..16 {
        let counter = CONTEXT_COMPRESS_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("context.md");
        let temp_path = path.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            counter
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            options.mode(permissions.mode());
        }
        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "failed to allocate context temp file",
    ))
}

#[cfg(unix)]
fn same_context_file_version(before: &Metadata, after: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.file_type().is_file()
        && after.file_type().is_file()
        && before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
}

#[cfg(not(unix))]
fn same_context_file_version(before: &Metadata, after: &Metadata) -> bool {
    before.file_type().is_file()
        && after.file_type().is_file()
        && before.len() == after.len()
        && before.modified().ok() == after.modified().ok()
}

#[cfg(unix)]
fn sync_context_parent(path: &Path) -> io::Result<()> {
    File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()
}

#[cfg(not(unix))]
fn sync_context_parent(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_changed_file_removes_owned_backup_and_allows_retry() {
        let root = std::env::temp_dir().join(format!(
            "prodex-context-compress-retry-{}-{}",
            std::process::id(),
            CONTEXT_COMPRESS_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create context root");
        let path = root.join("AGENTS.md");
        fs::write(
            &path,
            "This is actually a very verbose paragraph in order to make sure to reduce tokens.\n",
        )
        .expect("write original context");
        let changed = "This is actually a very verbose changed paragraph in order to make sure to reduce many tokens after a concurrent update.\n";
        let changed_path = path.clone();

        let error = compress_context_file_before_replace(&path, false, || {
            fs::write(&changed_path, changed).expect("change context before replace");
        })
        .expect_err("changed context must reject replacement");
        assert!(format!("{error:#}").contains("changed during compression"));
        assert!(!context_backup_path(&path).exists());
        assert_eq!(
            fs::read_to_string(&path).expect("read changed context"),
            changed
        );

        let retry = compress_context_file(&path, false).expect("retry compression");
        assert_eq!(retry.status, "compressed");
        assert_eq!(
            fs::read_to_string(context_backup_path(&path)).expect("read retry backup"),
            changed
        );
        let _ = fs::remove_dir_all(root);
    }
}
