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
    let agents_path = codex_home.join("AGENTS.md");
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

fn read_optional_link_target_contents(path: &Path) -> Result<String> {
    read_canonical_text_file_limited(path)
        .map(|contents| contents.unwrap_or_default())
        .with_context(|| format!("failed to read {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::ensure_agents_reference;
    use std::fs;

    #[test]
    fn keeps_unrelated_same_basename_reference_and_deduplicates_exact_reference() {
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("temp dir should resolve")
            .join(format!(
                "prodex-localization-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
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
}
