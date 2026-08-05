use anyhow::{Context, Result, bail};
use std::fs;
use std::io;
use std::path::Path;

use crate::fs_ops::{read_canonical_text_file_limited, read_text_file_limited, write_text_file};

pub(crate) fn localize_text_file(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let contents = read_optional_link_target_contents(path)?;
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
            write_text_file(path, &contents)?;
        }
        Ok(metadata) if metadata.is_dir() => {
            bail!("{} is a directory, expected a file", path.display());
        }
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            write_text_file(path, "")?;
        }
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", path.display()));
        }
    }
    Ok(())
}

pub(crate) fn ensure_agents_reference(codex_home: &Path, reference_path: &Path) -> Result<()> {
    let agents_path = effective_agents_path(codex_home)?;
    localize_text_file(&agents_path)?;
    let reference = format!("@{}", reference_path.display());
    let contents = read_text_file_limited(&agents_path)?.unwrap_or_default();
    let mut lines = Vec::new();
    let mut reference_seen = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        let is_reference = trimmed == reference;
        if is_reference {
            reference_seen = true;
        } else {
            lines.push(line);
        }
    }
    let cleaned = lines.join("\n");
    let updated = if cleaned.trim().is_empty() {
        format!("{reference}\n")
    } else {
        format!("{}\n\n{reference}\n", cleaned.trim_end())
    };
    if reference_seen && updated == contents {
        Ok(())
    } else {
        write_text_file(&agents_path, &updated)
    }
}

pub fn effective_agents_path(codex_home: &Path) -> Result<std::path::PathBuf> {
    let override_path = codex_home.join("AGENTS.override.md");
    if read_text_file_limited(&override_path)?
        .as_deref()
        .is_some_and(|contents| !contents.trim().is_empty())
    {
        Ok(override_path)
    } else {
        Ok(codex_home.join("AGENTS.md"))
    }
}

pub fn upsert_agents_block(
    codex_home: &Path,
    begin: &str,
    end: &str,
    block: &str,
) -> Result<std::path::PathBuf> {
    let path = effective_agents_path(codex_home)?;
    localize_text_file(&path)?;
    let contents = read_text_file_limited(&path)?.unwrap_or_default();
    let cleaned = without_marked_block(&contents, begin, end)?;
    let updated = if cleaned.trim().is_empty() {
        format!("{begin}\n{}\n{end}\n", block.trim())
    } else {
        format!(
            "{}\n\n{begin}\n{}\n{end}\n",
            cleaned.trim_end(),
            block.trim()
        )
    };
    write_text_file(&path, &updated)?;
    Ok(path)
}

pub fn remove_agents_block(codex_home: &Path, begin: &str, end: &str) -> Result<()> {
    for path in [
        codex_home.join("AGENTS.md"),
        codex_home.join("AGENTS.override.md"),
    ] {
        let Some(contents) = read_text_file_limited(&path)? else {
            continue;
        };
        let cleaned = without_marked_block(&contents, begin, end)?;
        if cleaned != contents {
            localize_text_file(&path)?;
            write_text_file(
                &path,
                if cleaned.trim().is_empty() {
                    ""
                } else {
                    cleaned.trim_end()
                },
            )?;
        }
    }
    Ok(())
}

fn without_marked_block(contents: &str, begin: &str, end: &str) -> Result<String> {
    let mut cleaned = contents.to_string();
    while let Some(start) = cleaned.find(begin) {
        let tail = &cleaned[start + begin.len()..];
        let end_offset = tail
            .find(end)
            .ok_or_else(|| anyhow::anyhow!("instruction block is missing its end marker"))?;
        let block_end = start + begin.len() + end_offset + end.len();
        cleaned.replace_range(start..block_end, "");
    }
    if cleaned.contains(end) {
        bail!("instruction block is missing its begin marker");
    }
    Ok(cleaned)
}

fn read_optional_link_target_contents(path: &Path) -> Result<String> {
    read_canonical_text_file_limited(path)
        .map(|contents| contents.unwrap_or_default())
        .with_context(|| format!("failed to read {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{ensure_agents_reference, remove_agents_block, upsert_agents_block};
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir()
            .canonicalize()
            .expect("temp dir should resolve")
            .join(format!(
                "prodex-localization-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
    }

    #[test]
    fn keeps_unrelated_same_basename_reference_and_deduplicates_exact_reference() {
        let root = temp_root("reference");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("AGENTS.md"),
            "@/home/test-user/unrelated/SUB_AGENTS.md\n",
        )
        .unwrap();
        let reference = root.join("SUB_AGENTS.md");
        ensure_agents_reference(&root, &reference).unwrap();

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        let expected = format!("@{}", reference.display());
        assert_eq!(
            agents
                .lines()
                .filter(|line| line.trim() == expected)
                .count(),
            1
        );
        assert!(agents.contains("@/home/test-user/unrelated/SUB_AGENTS.md"));
        ensure_agents_reference(&root, &reference).unwrap();
        assert_eq!(fs::read_to_string(root.join("AGENTS.md")).unwrap(), agents);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn marked_block_uses_nonempty_override_and_replaces_in_place() {
        let root = temp_root("block");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("AGENTS.md"), "base instructions\n").unwrap();
        fs::write(root.join("AGENTS.override.md"), "override instructions\n").unwrap();
        let begin = "<!-- TEST BEGIN -->";
        let end = "<!-- TEST END -->";
        let path = upsert_agents_block(&root, begin, end, "limit 4").unwrap();
        assert_eq!(path, root.join("AGENTS.override.md"));
        upsert_agents_block(&root, begin, end, "limit 16").unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents.matches(begin).count(), 1);
        assert!(contents.contains("limit 16"));
        assert!(!contents.contains("limit 4"));
        assert_eq!(
            fs::read_to_string(root.join("AGENTS.md")).unwrap(),
            "base instructions\n"
        );
        remove_agents_block(&root, begin, end).unwrap();
        assert!(!fs::read_to_string(path).unwrap().contains(begin));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_override_is_skipped() {
        let root = temp_root("empty-override");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("AGENTS.override.md"), " \n").unwrap();
        let path = upsert_agents_block(&root, "<!-- B -->", "<!-- E -->", "body").unwrap();
        assert_eq!(path, root.join("AGENTS.md"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn marked_block_selects_the_effective_file_when_only_one_or_neither_exists() {
        for (index, agents, override_agents, expected) in [
            (0, Some("base instructions\n"), None, "AGENTS.md"),
            (
                1,
                None,
                Some("override instructions\n"),
                "AGENTS.override.md",
            ),
            (2, None, None, "AGENTS.md"),
        ] {
            let root = temp_root(&format!("effective-{index}"));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            if let Some(contents) = agents {
                fs::write(root.join("AGENTS.md"), contents).unwrap();
            }
            if let Some(contents) = override_agents {
                fs::write(root.join("AGENTS.override.md"), contents).unwrap();
            }

            let begin = "<!-- PRODEX SUB-AGENT BEGIN -->";
            let end = "<!-- PRODEX SUB-AGENT END -->";
            let path = upsert_agents_block(&root, begin, end, "complete instructions").unwrap();
            assert_eq!(path, root.join(expected));
            let contents = fs::read_to_string(&path).unwrap();
            assert!(contents.contains("complete instructions"));
            assert!(!contents.contains("@SUB_AGENTS.md"));
            remove_agents_block(&root, begin, end).unwrap();
            assert!(!fs::read_to_string(path).unwrap().contains(begin));
            fs::remove_dir_all(root).unwrap();
        }
    }
}
