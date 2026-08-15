use crate::app_state::AppStateIoExt;
use anyhow::{Result, bail};
use prodex_cli::SuperExternalProvider;
use prodex_core::AppPaths;
use prodex_provider_core::ProviderId;
use prodex_state::AppState;
use std::ffi::OsString;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeResumeSessionSettings {
    pub(super) model: Option<String>,
    pub(super) reasoning_effort: Option<String>,
}

pub(super) fn runtime_resume_external_provider_from_codex_args(
    codex_args: &[OsString],
) -> Result<Option<SuperExternalProvider>> {
    Ok(runtime_resume_provider_from_codex_args(codex_args)?
        .and_then(SuperExternalProvider::from_provider_id))
}

pub(in crate::app_commands) fn runtime_resume_provider_from_codex_args(
    codex_args: &[OsString],
) -> Result<Option<ProviderId>> {
    let Some(report) = runtime_resume_session_report_from_codex_args(codex_args)? else {
        return Ok(None);
    };
    resolve_bound_provider_identity(report.model_provider.as_deref())
}

pub(super) fn runtime_resume_session_settings_from_codex_args(
    codex_args: &[OsString],
) -> Option<RuntimeResumeSessionSettings> {
    runtime_resume_session_report_from_codex_args(codex_args)
        .ok()
        .flatten()
        .map(|report| RuntimeResumeSessionSettings {
            model: report.last_model().map(ToOwned::to_owned),
            reasoning_effort: report.last_reasoning_effort().map(ToOwned::to_owned),
        })
}

fn runtime_resume_session_report_from_codex_args(
    codex_args: &[OsString],
) -> Result<Option<prodex_session_store::SessionReport>> {
    let normalized = prodex_runtime_launch::normalize_run_codex_args(codex_args);
    let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(&normalized) else {
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
        Err(prodex_session_store::SessionResolveError::Missing { .. }) => return Ok(None),
        Err(prodex_session_store::SessionResolveError::Ambiguous { .. }) => {
            bail!("resume target is ambiguous; use the full session UUID")
        }
    };
    Ok(Some(report))
}

fn resolve_bound_provider_identity(value: Option<&str>) -> Result<Option<ProviderId>> {
    let Some(value) = value else {
        return Ok(None);
    };
    prodex_provider_core::provider_implementation_registry()
        .resolve_model_provider_id(value)
        .map(Some)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "resumed session has an unsupported provider identity; configure the matching provider or start a fresh session"
            )
        })
}

#[cfg(test)]
fn runtime_external_provider_from_model_provider_id(
    model_provider: &str,
) -> Option<SuperExternalProvider> {
    prodex_provider_core::provider_implementation_registry()
        .resolve_model_provider_id(model_provider)
        .and_then(SuperExternalProvider::from_provider_id)
}

#[cfg(test)]
mod tests {
    use super::{
        SuperExternalProvider, resolve_bound_provider_identity,
        runtime_external_provider_from_model_provider_id,
    };

    #[test]
    fn runtime_external_provider_from_model_provider_id_accepts_kiro() {
        assert_eq!(
            runtime_external_provider_from_model_provider_id(prodex_cli::SUPER_KIRO_PROVIDER_ID),
            Some(SuperExternalProvider::Kiro)
        );
    }

    #[test]
    fn unknown_bound_provider_fails_instead_of_becoming_unbound() {
        assert_eq!(resolve_bound_provider_identity(None).unwrap(), None);
        assert_eq!(
            resolve_bound_provider_identity(Some(prodex_cli::SUPER_KIRO_PROVIDER_ID)).unwrap(),
            Some(prodex_provider_core::ProviderId::Kiro)
        );
        let error = resolve_bound_provider_identity(Some("unknown-provider"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("unsupported provider identity"), "{error}");
        assert!(!error.contains("unknown-provider"), "{error}");
    }
}
