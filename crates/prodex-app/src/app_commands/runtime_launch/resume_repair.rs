use super::*;

pub(super) fn repair_resume_session_metadata_prefix_from_codex_args(
    codex_args: &[OsString],
) -> Result<()> {
    let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(codex_args) else {
        return Ok(());
    };
    let paths = AppPaths::discover()?;
    let _ = prodex_session_store::repair_resume_session_metadata_prefix(
        &paths.shared_codex_root,
        session_id,
    )?;
    if let Some(path) = prodex_session_store::find_unrepairable_resume_session(
        &paths.shared_codex_root,
        session_id,
    )? {
        bail!(
            "session '{}' cannot be resumed because {} does not contain session metadata; the file is too incomplete to repair",
            session_id,
            path.display()
        );
    }
    Ok(())
}
