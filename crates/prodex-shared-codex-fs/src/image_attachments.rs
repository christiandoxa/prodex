use super::*;

const SESSION_IMAGE_ATTACHMENT_DIR: &str = "image_attachments";
const CODEX_IMAGE_TAG_PREFIX: &str = "<image ";
const CODEX_IMAGE_PATH_PREFIX: &str = r#"path=""#;
const CODEX_IMAGE_PATH_ESCAPED_PREFIX: &str = r#"path=\""#;
const CODEX_IMAGE_PATH_QUOTE: &str = r#"""#;
const CODEX_IMAGE_PATH_ESCAPED_QUOTE: &str = r#"\""#;
const CODEX_CLIPBOARD_PREFIX: &str = "codex-clipboard-";

pub fn persist_codex_session_image_attachments(codex_home: &Path) -> Result<()> {
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

    for entry in fs::read_dir(sessions_dir)
        .with_context(|| format!("failed to read {}", sessions_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", sessions_dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read metadata for {}", path.display()))?;
        if file_type.is_dir() {
            persist_codex_session_image_attachments_in_dir(codex_home, &path)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        {
            persist_codex_session_file_image_attachments(codex_home, &path)?;
        }
    }

    Ok(())
}

fn persist_codex_session_file_image_attachments(
    codex_home: &Path,
    session_file: &Path,
) -> Result<()> {
    let contents = fs::read_to_string(session_file)
        .with_context(|| format!("failed to read {}", session_file.display()))?;
    let rewritten = rewrite_codex_session_image_paths(codex_home, &contents)?;
    if rewritten != contents {
        fs::write(session_file, rewritten)
            .with_context(|| format!("failed to write {}", session_file.display()))?;
    }
    Ok(())
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
            cursor = tag_end;
            continue;
        };
        let path_start = tag_start + relative_path_start + path_prefix.len();
        let Some(relative_path_end) = contents[path_start..tag_end].find(path_quote) else {
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
    if !source.is_absolute() || !source.is_file() {
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
    if !destination.is_file() {
        copy_shared_codex_file(source, &destination)?;
    }
    Ok(Some(destination.display().to_string()))
}

#[cfg(test)]
#[path = "../tests/src/image_attachments.rs"]
mod tests;
