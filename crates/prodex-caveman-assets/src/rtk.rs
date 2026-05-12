use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::localization::localize_text_file;
use crate::{AGENTS_MD, PRODEX_RTK_CODEX_AWARENESS, RTK_MD};

pub fn configure_rtk_codex_home(codex_home: &Path) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let rtk_md_path = codex_home.join(RTK_MD);
    fs::write(&rtk_md_path, PRODEX_RTK_CODEX_AWARENESS)
        .with_context(|| format!("failed to write {}", rtk_md_path.display()))?;
    localize_text_file(&codex_home.join(AGENTS_MD))?;
    ensure_rtk_agents_reference(codex_home, &rtk_md_path)
}

fn ensure_rtk_agents_reference(codex_home: &Path, rtk_md_path: &Path) -> Result<()> {
    let agents_path = codex_home.join(AGENTS_MD);
    let reference = format!("@{}", rtk_md_path.display());
    let contents = fs::read_to_string(&agents_path)
        .with_context(|| format!("failed to read {}", agents_path.display()))?;
    if contents.lines().any(|line| line.trim() == reference) {
        return Ok(());
    }

    let mut updated = String::new();
    if contents.trim().is_empty() {
        updated.push_str(&reference);
        updated.push('\n');
    } else {
        updated.push_str(contents.trim_end());
        updated.push_str("\n\n");
        updated.push_str(&reference);
        updated.push('\n');
    }
    fs::write(&agents_path, updated)
        .with_context(|| format!("failed to write {}", agents_path.display()))?;
    Ok(())
}
