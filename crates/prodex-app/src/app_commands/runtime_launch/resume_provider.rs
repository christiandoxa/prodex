use super::*;

pub(super) fn runtime_resume_external_provider_from_codex_args(
    codex_args: &[OsString],
) -> Result<Option<SuperExternalProvider>> {
    let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(codex_args) else {
        return Ok(None);
    };
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let report = match prodex_session_store::resolve_session_report_by_id_in_store(
        &paths.shared_codex_root,
        &state,
        session_id,
    ) {
        Ok(report) => report,
        Err(prodex_session_store::SessionResolveError::Missing { .. })
        | Err(prodex_session_store::SessionResolveError::Ambiguous { .. }) => return Ok(None),
    };
    Ok(report
        .model_provider
        .as_deref()
        .and_then(runtime_external_provider_from_model_provider_id))
}

fn runtime_external_provider_from_model_provider_id(
    model_provider: &str,
) -> Option<SuperExternalProvider> {
    let model_provider = model_provider.trim();
    if model_provider.eq_ignore_ascii_case(SUPER_GEMINI_PROVIDER_ID) {
        return Some(SuperExternalProvider::Gemini);
    }
    if model_provider.eq_ignore_ascii_case(SUPER_ANTHROPIC_PROVIDER_ID) {
        return Some(SuperExternalProvider::Anthropic);
    }
    if model_provider.eq_ignore_ascii_case(SUPER_COPILOT_PROVIDER_ID) {
        return Some(SuperExternalProvider::Copilot);
    }
    if model_provider.eq_ignore_ascii_case(SUPER_DEEPSEEK_PROVIDER_ID) {
        return Some(SuperExternalProvider::DeepSeek);
    }
    if model_provider.eq_ignore_ascii_case(prodex_cli::SUPER_KIRO_PROVIDER_ID) {
        return Some(SuperExternalProvider::Kiro);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_external_provider_from_model_provider_id_accepts_kiro() {
        assert_eq!(
            runtime_external_provider_from_model_provider_id(prodex_cli::SUPER_KIRO_PROVIDER_ID),
            Some(SuperExternalProvider::Kiro)
        );
    }
}
