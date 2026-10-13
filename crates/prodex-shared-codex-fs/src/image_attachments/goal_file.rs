//! Only generated goal wrappers may contain stale nested attachment addresses.
//! The raw pasted instructions and image payloads must remain byte-exact.
use super::*;

pub(crate) fn persist_codex_goal_objective_attachments(
    codex_home: &Path,
    objective: &str,
) -> Result<()> {
    let attachment_root = codex_home.join(SESSION_ATTACHMENT_DIR);
    for path in codex_session_persisted_attachment_paths(objective) {
        if generated_goal_path_is_safe(&attachment_root, &path)? {
            repair_generated_goal_file(codex_home, &path)?;
        }
    }
    Ok(())
}

fn generated_goal_path_is_safe(attachment_root: &Path, path: &Path) -> Result<bool> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(false);
    };
    let generated = prodex_mojo_core::shared_attachment_policy::goal_objective_file_name(name)
        .map_err(|error| {
            anyhow::anyhow!("Mojo goal attachment classification failed: {error:?}")
        })?;
    if !generated || !path.starts_with(attachment_root) || !path_is_regular_file(path)? {
        return Ok(false);
    }
    // Shared storage can itself be linked, but an attachment ID must not
    // redirect filesystem writes to arbitrary user files outside that root.
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    Ok(!fs::symlink_metadata(parent)?.file_type().is_symlink()
        && prodex_core::path_is_under_root(attachment_root, path))
}

fn repair_generated_goal_file(codex_home: &Path, path: &Path) -> Result<()> {
    let Some(contents) = read_codex_session_attachment_file(path)? else {
        return Ok(());
    };
    let rewritten = rewrite_codex_persisted_attachment_paths(codex_home, &contents)?;
    if rewritten == contents {
        return Ok(());
    }
    // Native goal materialization creates immutable wrapper files. Avoid
    // overwriting any user edit observed since our bounded no-follow read.
    if read_codex_session_attachment_file(path)?.as_deref() != Some(&contents) {
        return Ok(());
    }
    write_codex_session_attachment_file(path, &rewritten)
}
