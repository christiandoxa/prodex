use super::*;
use std::io::{Read, Write};

const CODEX_SESSION_ATTACHMENT_REWRITE_MAX_BYTES: u64 = 64 * 1024 * 1024;

const SESSION_IMAGE_ATTACHMENT_DIR: &str = "image_attachments";
const SESSION_ATTACHMENT_DIR: &str = "attachments";
const CODEX_ATTACHMENT_PATH_MARKER: &str = "/attachments/";
const CODEX_PASTED_TEXT_PREFIX: &str = "pasted-text-";
const CODEX_ATTACHMENT_IMAGE_PREFIX: &str = "image-";
const CODEX_GOAL_OBJECTIVE_FILE: &str = "goal-objective.md";
const CODEX_IMAGE_TAG_PREFIX: &str = "<image ";
const CODEX_IMAGE_PATH_PREFIX: &str = r#"path=""#;
const CODEX_IMAGE_PATH_ESCAPED_PREFIX: &str = r#"path=\""#;
const CODEX_IMAGE_PATH_QUOTE: &str = r#"""#;
const CODEX_IMAGE_PATH_ESCAPED_QUOTE: &str = r#"\""#;
const CODEX_CLIPBOARD_PREFIX: &str = "codex-clipboard-";

pub fn persist_codex_session_image_attachments(codex_home: &Path) -> Result<()> {
    let Some(_maintenance_lock) = try_lock_codex_session_maintenance(codex_home)? else {
        return Ok(());
    };
    persist_codex_session_image_attachments_in_dir(codex_home, &codex_home.join("sessions"))?;
    persist_codex_session_image_attachments_in_dir(
        codex_home,
        &codex_home.join("archived_sessions"),
    )
}

fn persist_codex_session_image_attachments_in_dir(
    codex_home: &Path,
    sessions_dir: &Path,
) -> Result<()> {
    if !sessions_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(sessions_dir).context("failed to read codex sessions directory")? {
        let entry = entry.context("failed to read codex sessions directory entry")?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .context("failed to read codex session entry metadata")?;
        if file_type.is_dir() {
            persist_codex_session_image_attachments_in_dir(codex_home, &path)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        {
            let _ = persist_codex_session_file_image_attachments(codex_home, &path)?;
        }
    }

    Ok(())
}

pub(crate) fn persist_codex_session_file_image_attachments(
    codex_home: &Path,
    session_file: &Path,
) -> Result<Option<String>> {
    let Some(contents) = read_codex_session_attachment_file(session_file)? else {
        return Ok(None);
    };
    let rewritten = rewrite_codex_persisted_attachment_paths(codex_home, &contents)?;
    if rewritten.len() as u64 > CODEX_SESSION_ATTACHMENT_REWRITE_MAX_BYTES {
        return Ok(None);
    }
    if rewritten != contents {
        write_codex_session_attachment_file(session_file, &rewritten)?;
    }
    Ok(Some(rewritten))
}

fn read_codex_session_attachment_file(path: &Path) -> Result<Option<String>> {
    let metadata = fs::symlink_metadata(path).context("failed to read codex session file")?;
    if metadata.file_type().is_symlink() {
        bail!("refusing to read codex session file through symlink");
    }
    if !metadata.file_type().is_file() {
        bail!("codex session path is not a file");
    }
    if metadata.len() > CODEX_SESSION_ATTACHMENT_REWRITE_MAX_BYTES {
        return Ok(None);
    }
    let file = fs::File::open(path).context("failed to read codex session file")?;
    let opened_metadata = file
        .metadata()
        .context("failed to read codex session file metadata")?;
    if !codex_session_same_file_metadata(&metadata, &opened_metadata) {
        bail!("codex session file changed while opening");
    }
    let mut bytes = Vec::new();
    file.take(CODEX_SESSION_ATTACHMENT_REWRITE_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .context("failed to read codex session file")?;
    if bytes.len() as u64 > CODEX_SESSION_ATTACHMENT_REWRITE_MAX_BYTES {
        return Ok(None);
    }
    String::from_utf8(bytes)
        .map(Some)
        .context("failed to decode codex session file")
}

fn write_codex_session_attachment_file(path: &Path, contents: &str) -> Result<()> {
    if contents.len() as u64 > CODEX_SESSION_ATTACHMENT_REWRITE_MAX_BYTES {
        bail!(
            "rewritten codex session exceeds safe size limit ({} bytes)",
            CODEX_SESSION_ATTACHMENT_REWRITE_MAX_BYTES
        );
    }
    let temp_path = path.with_extension(format!(
        "{}.prodex-attachments-tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("jsonl"),
        std::process::id()
    ));
    let _ = fs::remove_file(&temp_path);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(
            fs::metadata(path)
                .context("failed to read codex session file permissions")?
                .permissions()
                .mode(),
        );
    }
    let mut file = options
        .open(&temp_path)
        .context("failed to write codex session file")?;
    file.write_all(contents.as_bytes())
        .context("failed to write codex session file")?;
    file.sync_all()
        .context("failed to sync codex session file")?;
    fs::rename(&temp_path, path).context("failed to write codex session file")?;
    #[cfg(unix)]
    fs::File::open(path.parent().unwrap_or_else(|| Path::new(".")))
        .and_then(|directory| directory.sync_all())
        .context("failed to sync codex session directory")?;
    Ok(())
}

#[cfg(unix)]
fn codex_session_same_file_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn codex_session_same_file_metadata(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
}

pub(crate) fn codex_session_image_attachments_are_stable(
    codex_home: &Path,
    contents: &str,
) -> bool {
    let stable_image_dir = codex_home.join(SESSION_IMAGE_ATTACHMENT_DIR);
    let stable_attachment_dir = codex_home.join(SESSION_ATTACHMENT_DIR);
    let mut cursor = 0;

    while let Some(relative_tag_start) = contents[cursor..].find(CODEX_IMAGE_TAG_PREFIX) {
        let tag_start = cursor + relative_tag_start;
        let Some(relative_tag_end) = contents[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end;
        let tag = &contents[tag_start..tag_end];
        let Some((relative_path_start, path_prefix, path_quote)) = image_tag_path_attr(tag) else {
            cursor = tag_end;
            continue;
        };
        let path_start = tag_start + relative_path_start + path_prefix.len();
        let Some(relative_path_end) = contents[path_start..tag_end].find(path_quote) else {
            cursor = tag_end;
            continue;
        };
        let raw_path = &contents[path_start..path_start + relative_path_end];
        let path = Path::new(raw_path);
        let is_clipboard_path = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(CODEX_CLIPBOARD_PREFIX));
        if path.is_absolute() && is_clipboard_path && !path.starts_with(&stable_image_dir) {
            return false;
        }
        cursor = tag_end;
    }

    let mut cursor = 0;
    while let Some((path_start, path_end)) = next_codex_session_clipboard_path(contents, cursor) {
        let path = Path::new(&contents[path_start..path_end]);
        if path.is_absolute() && !path.starts_with(&stable_image_dir) {
            return false;
        }
        cursor = path_end;
    }

    let mut cursor = 0;
    while let Some((path_start, path_end)) = next_codex_session_attachment_path(contents, cursor) {
        let path = Path::new(&contents[path_start..path_end]);
        if path.is_absolute()
            && codex_attachment_path_suffix(path).is_some()
            && !path.starts_with(&stable_attachment_dir)
        {
            return false;
        }
        cursor = path_end;
    }

    true
}

pub(crate) fn rewrite_codex_persisted_attachment_paths(
    codex_home: &Path,
    contents: &str,
) -> Result<String> {
    let image_rewritten = rewrite_codex_session_image_paths(codex_home, contents)?;
    let clipboard_rewritten =
        rewrite_codex_session_inline_clipboard_paths(codex_home, &image_rewritten)?;
    rewrite_codex_session_inline_attachment_paths(codex_home, &clipboard_rewritten)
}

fn rewrite_codex_session_image_paths(codex_home: &Path, contents: &str) -> Result<String> {
    let mut output = String::with_capacity(contents.len());
    let mut cursor = 0;

    while let Some(relative_tag_start) = contents[cursor..].find(CODEX_IMAGE_TAG_PREFIX) {
        let tag_start = cursor + relative_tag_start;
        let Some(relative_tag_end) = contents[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end;
        let tag = &contents[tag_start..tag_end];
        let Some((relative_path_start, path_prefix, path_quote)) = image_tag_path_attr(tag) else {
            output.push_str(&contents[cursor..tag_end]);
            cursor = tag_end;
            continue;
        };
        let path_start = tag_start + relative_path_start + path_prefix.len();
        let Some(relative_path_end) = contents[path_start..tag_end].find(path_quote) else {
            output.push_str(&contents[cursor..tag_end]);
            cursor = tag_end;
            continue;
        };
        let path_end = path_start + relative_path_end;
        let raw_path = &contents[path_start..path_end];
        let replacement = stable_codex_session_image_path(codex_home, raw_path)?;

        output.push_str(&contents[cursor..path_start]);
        output.push_str(replacement.as_deref().unwrap_or(raw_path));
        cursor = path_end;
    }

    output.push_str(&contents[cursor..]);
    Ok(output)
}

fn image_tag_path_attr(tag: &str) -> Option<(usize, &'static str, &'static str)> {
    tag.find(CODEX_IMAGE_PATH_ESCAPED_PREFIX)
        .map(|offset| {
            (
                offset,
                CODEX_IMAGE_PATH_ESCAPED_PREFIX,
                CODEX_IMAGE_PATH_ESCAPED_QUOTE,
            )
        })
        .or_else(|| {
            tag.find(CODEX_IMAGE_PATH_PREFIX)
                .map(|offset| (offset, CODEX_IMAGE_PATH_PREFIX, CODEX_IMAGE_PATH_QUOTE))
        })
}

fn stable_codex_session_image_path(codex_home: &Path, raw_path: &str) -> Result<Option<String>> {
    let source = Path::new(raw_path);
    if !source.is_absolute() {
        return Ok(None);
    }
    let Some(file_name) = source.file_name().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    if !file_name.starts_with(CODEX_CLIPBOARD_PREFIX) {
        return Ok(None);
    }

    let destination = codex_home
        .join(SESSION_IMAGE_ATTACHMENT_DIR)
        .join(file_name);
    if !path_is_regular_file(&destination)? {
        if !codex_clipboard_source_is_persistable(source) || !path_is_regular_file(source)? {
            return Ok(None);
        }
        copy_shared_codex_file(source, &destination)?;
    }
    Ok(Some(destination.display().to_string()))
}

fn rewrite_codex_session_inline_attachment_paths(
    codex_home: &Path,
    contents: &str,
) -> Result<String> {
    let mut output = String::with_capacity(contents.len());
    let mut cursor = 0;

    while let Some((path_start, path_end)) = next_codex_session_attachment_path(contents, cursor) {
        let raw_path = &contents[path_start..path_end];
        let replacement = stable_codex_session_attachment_path(codex_home, raw_path)?;
        output.push_str(&contents[cursor..path_start]);
        output.push_str(replacement.as_deref().unwrap_or(raw_path));
        cursor = path_end;
    }

    output.push_str(&contents[cursor..]);
    Ok(output)
}

fn rewrite_codex_session_inline_clipboard_paths(
    codex_home: &Path,
    contents: &str,
) -> Result<String> {
    let mut output = String::with_capacity(contents.len());
    let mut cursor = 0;

    while let Some((path_start, path_end)) = next_codex_session_clipboard_path(contents, cursor) {
        let raw_path = &contents[path_start..path_end];
        let replacement = stable_codex_session_image_path(codex_home, raw_path)?;
        output.push_str(&contents[cursor..path_start]);
        output.push_str(replacement.as_deref().unwrap_or(raw_path));
        cursor = path_end;
    }

    output.push_str(&contents[cursor..]);
    Ok(output)
}

fn next_codex_session_clipboard_path(contents: &str, cursor: usize) -> Option<(usize, usize)> {
    let marker_start = cursor + contents[cursor..].find(CODEX_CLIPBOARD_PREFIX)?;
    let bytes = contents.as_bytes();

    let mut path_start = marker_start;
    while path_start > 0 && is_codex_session_path_byte(bytes[path_start - 1]) {
        path_start -= 1;
    }

    let mut path_end = marker_start + CODEX_CLIPBOARD_PREFIX.len();
    while path_end < bytes.len() && is_codex_session_path_byte(bytes[path_end]) {
        path_end += 1;
    }
    while path_end > marker_start && bytes[path_end - 1] == b'.' {
        path_end -= 1;
    }

    (path_start < marker_start && path_end > marker_start + CODEX_CLIPBOARD_PREFIX.len())
        .then_some((path_start, path_end))
}

fn next_codex_session_attachment_path(contents: &str, cursor: usize) -> Option<(usize, usize)> {
    let marker_start = cursor + contents[cursor..].find(CODEX_ATTACHMENT_PATH_MARKER)?;
    let bytes = contents.as_bytes();

    let mut path_start = marker_start;
    while path_start > 0 && is_codex_session_path_byte(bytes[path_start - 1]) {
        path_start -= 1;
    }

    let mut path_end = marker_start + CODEX_ATTACHMENT_PATH_MARKER.len();
    while path_end < bytes.len() && is_codex_session_path_byte(bytes[path_end]) {
        path_end += 1;
    }
    while path_end > marker_start && bytes[path_end - 1] == b'.' {
        path_end -= 1;
    }

    (path_start < marker_start && path_end > marker_start + CODEX_ATTACHMENT_PATH_MARKER.len())
        .then_some((path_start, path_end))
}

fn is_codex_session_path_byte(byte: u8) -> bool {
    !matches!(
        byte,
        b'\\'
            | b'"'
            | b'\''
            | b'<'
            | b'>'
            | b'('
            | b')'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b','
            | b';'
            | b' '
            | b'\t'
            | b'\r'
            | b'\n'
    )
}

fn stable_codex_session_attachment_path(
    codex_home: &Path,
    raw_path: &str,
) -> Result<Option<String>> {
    let source = Path::new(raw_path);
    if !source.is_absolute() {
        return Ok(None);
    }
    let Some(relative_attachment_path) = codex_attachment_path_suffix(source) else {
        return Ok(None);
    };
    let destination = codex_home
        .join(SESSION_ATTACHMENT_DIR)
        .join(relative_attachment_path);
    if !path_is_regular_file(&destination)? {
        if !path_is_regular_file(source)? {
            return Ok(None);
        }
        copy_shared_codex_file(source, &destination)?;
    }
    Ok(Some(destination.display().to_string()))
}

fn codex_attachment_path_suffix(path: &Path) -> Option<PathBuf> {
    let components: Vec<_> = path.components().collect();
    let attachment_index = components
        .iter()
        .position(|component| component.as_os_str().to_str() == Some(SESSION_ATTACHMENT_DIR))?;
    let id_component = components.get(attachment_index + 1)?;
    let file_component = components.get(attachment_index + 2)?;
    if components.get(attachment_index + 3).is_some() {
        return None;
    }
    let id = id_component.as_os_str().to_str()?;
    if id.trim().is_empty() || id.contains(std::path::MAIN_SEPARATOR) || id == "." || id == ".." {
        return None;
    }
    let file_name = file_component.as_os_str().to_str()?;
    if file_name.trim().is_empty()
        || file_name.contains(std::path::MAIN_SEPARATOR)
        || file_name == "."
        || file_name == ".."
        || !codex_attachment_file_name_is_persistable(file_name)
    {
        return None;
    }
    Some(PathBuf::from(id_component.as_os_str()).join(file_component.as_os_str()))
}

fn codex_attachment_file_name_is_persistable(file_name: &str) -> bool {
    file_name.starts_with(CODEX_PASTED_TEXT_PREFIX)
        || file_name.starts_with(CODEX_ATTACHMENT_IMAGE_PREFIX)
        || file_name == CODEX_GOAL_OBJECTIVE_FILE
}

fn codex_clipboard_source_is_persistable(source: &Path) -> bool {
    prodex_core::path_is_under_root(&env::temp_dir(), source)
}

fn path_is_regular_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_file()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

#[cfg(test)]
#[path = "../tests/src/image_attachments.rs"]
mod tests;
