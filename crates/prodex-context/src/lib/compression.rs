use super::*;
use std::ffi::{OsStr, OsString};
use std::fs::{File, Metadata};
use std::io::{self, Read as _, Write as _};
use std::sync::atomic::{AtomicU64, Ordering};

const CONTEXT_COMPRESS_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
static CONTEXT_COMPRESS_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

mod context_parent;
use context_parent::ContextParent;

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

    let (original, original_metadata, parent, source_name) = read_context_file_no_follow(path)?;
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
    let backup_path = context_backup_path(path, &parent)?;
    let backup_name = backup_path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "context backup name is empty")
    })?;

    if parent.entry_exists(backup_name)? {
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
            &parent,
            backup_name,
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
        if let Err(replace_error) = replace_context_file_if_unchanged(
            &parent,
            &source_name,
            compressed.as_bytes(),
            &original_metadata,
            before_replace,
        ) {
            if let Err(cleanup_error) = parent.remove_if_owned(backup_name, &backup_metadata) {
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
        parent
            .sync()
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
        append_context_compressed_line(
            line,
            &mut output,
            &mut paragraph,
            &mut in_fence,
            &mut previous_blank,
        );
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

fn append_context_compressed_line(
    line: &str,
    output: &mut Vec<String>,
    paragraph: &mut Vec<String>,
    in_fence: &mut bool,
    previous_blank: &mut bool,
) {
    let trimmed = line.trim();
    let fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
    if fence {
        flush_context_paragraph(paragraph, output);
        output.push(line.to_string());
        *in_fence = !*in_fence;
        *previous_blank = false;
        return;
    }

    if *in_fence || protected_context_line(line) {
        flush_context_paragraph(paragraph, output);
        output.push(line.to_string());
        *previous_blank = false;
        return;
    }

    if trimmed.is_empty() {
        flush_context_paragraph(paragraph, output);
        if !*previous_blank && !output.is_empty() {
            output.push(String::new());
        }
        *previous_blank = true;
        return;
    }

    paragraph.push(trimmed.to_string());
    *previous_blank = false;
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

fn context_backup_path(path: &Path, parent: &ContextParent) -> io::Result<PathBuf> {
    let path_parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("context");
    let legacy = path_parent.join(format!("{stem}.original.md"));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("context");
    let qualified = path_parent.join(format!("{file_name}.original.md"));

    let qualified_name = qualified.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "qualified context backup name is empty",
        )
    })?;
    if parent.entry_exists(qualified_name)?
        || (path.extension().and_then(|extension| extension.to_str()) != Some("md")
            && has_same_stem_context_file(path, path_parent))
    {
        Ok(qualified)
    } else {
        Ok(legacy)
    }
}

fn has_same_stem_context_file(path: &Path, parent: &Path) -> bool {
    let Some(stem) = path.file_stem() else {
        return false;
    };
    let directory = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };

    // ponytail: one directory scan per source keeps legacy backup names compatible; precompute stems if this scales up.
    fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .any(|candidate| {
            candidate != path
                && candidate
                    .file_stem()
                    .is_some_and(|candidate_stem| same_context_stem(candidate_stem, stem))
                && is_compressible_context_file(&candidate)
        })
}

fn same_context_stem(left: &OsStr, right: &OsStr) -> bool {
    match (left.to_str(), right.to_str()) {
        (Some(left), Some(right)) => left.to_lowercase() == right.to_lowercase(),
        _ => left == right,
    }
}

fn read_context_file_no_follow(path: &Path) -> Result<(String, Metadata, ContextParent, OsString)> {
    let (parent, name) = ContextParent::open_for(path)
        .with_context(|| format!("failed to open context parent {}", path.display()))?;
    let file = parent
        .open_existing(&name)
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
    Ok((original, metadata, parent, name))
}

fn write_context_backup_create_new(
    parent: &ContextParent,
    name: &OsStr,
    bytes: &[u8],
    permissions: fs::Permissions,
) -> io::Result<Metadata> {
    let mut file = parent.create_new(name, &permissions)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = parent.remove_entry(name);
        return Err(error);
    }
    if let Err(error) = file
        .set_permissions(permissions)
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = parent.remove_entry(name);
        return Err(error);
    }
    let metadata = file.metadata()?;
    if let Err(error) = parent.sync() {
        drop(file);
        let _ = parent.remove_entry(name);
        let _ = parent.sync();
        return Err(error);
    }
    Ok(metadata)
}

fn replace_context_file_if_unchanged(
    parent: &ContextParent,
    source_name: &OsStr,
    bytes: &[u8],
    original_metadata: &Metadata,
    before_commit: impl FnOnce(),
) -> io::Result<()> {
    let current_metadata = parent.open_existing(source_name)?.metadata()?;
    if !same_context_file_version(original_metadata, &current_metadata) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "context file changed during compression",
        ));
    }
    let (temp_name, mut temp_file) =
        create_context_temp_file(parent, source_name, original_metadata.permissions())?;
    if let Err(error) = temp_file
        .write_all(bytes)
        .and_then(|()| temp_file.sync_all())
    {
        drop(temp_file);
        let _ = parent.remove_entry(&temp_name);
        return Err(error);
    }
    if let Err(error) = temp_file
        .set_permissions(original_metadata.permissions())
        .and_then(|()| temp_file.sync_all())
    {
        drop(temp_file);
        let _ = parent.remove_entry(&temp_name);
        return Err(error);
    }
    let current_metadata = parent.open_existing(source_name)?.metadata()?;
    if !same_context_file_version(original_metadata, &current_metadata) {
        let _ = parent.remove_entry(&temp_name);
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "context file changed during compression",
        ));
    }
    before_commit();
    if let Err(error) = parent.replace(source_name, &temp_name, original_metadata, &temp_file) {
        let _ = parent.remove_entry(&temp_name);
        return Err(error);
    }
    drop(temp_file);
    Ok(())
}

fn create_context_temp_file(
    parent: &ContextParent,
    source_name: &OsStr,
    permissions: fs::Permissions,
) -> io::Result<(OsString, File)> {
    for _ in 0..16 {
        let counter = CONTEXT_COMPRESS_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let file_name = source_name.to_string_lossy();
        let temp_name = OsString::from(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            counter
        ));
        match parent.create_new(&temp_name, &permissions) {
            Ok(file) => return Ok((temp_name, file)),
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
        assert!(
            format!("{error:#}").contains("changed during compression"),
            "{error:#}"
        );
        assert!(!root.join("AGENTS.original.md").exists());
        assert_eq!(
            fs::read_to_string(&path).expect("read changed context"),
            changed
        );

        let retry = compress_context_file(&path, false).expect("retry compression");
        assert_eq!(retry.status, "compressed");
        assert_eq!(
            fs::read_to_string(root.join("AGENTS.original.md")).expect("read retry backup"),
            changed
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn replacement_race_does_not_overwrite_replaced_source() {
        let root = std::env::temp_dir().join(format!(
            "prodex-context-compress-race-{}-{}",
            std::process::id(),
            CONTEXT_COMPRESS_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create context root");
        let path = root.join("AGENTS.md");
        let writer_path = root.join("writer.md");
        fs::write(
            &path,
            "This is actually a very verbose paragraph in order to make sure to reduce tokens.\n",
        )
        .expect("write original context");
        let writer = "This is the concurrent writer's content and it must survive replacement.\n";
        fs::write(&writer_path, writer).expect("write concurrent context");

        let error = compress_context_file_before_replace(&path, false, || {
            fs::rename(&writer_path, &path).expect("replace source during race window");
        })
        .expect_err("replacement race must reject compression");
        assert!(format!("{error:#}").contains("changed during compression"));
        assert_eq!(
            fs::read_to_string(&path).expect("read concurrent context"),
            writer
        );
        assert!(!root.join("AGENTS.original.md").exists());
        let _ = fs::remove_dir_all(root);
    }
}
