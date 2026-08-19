use crate::app_state::AppStateIoExt;
use anyhow::{Result, bail};
use prodex_cli::SuperExternalProvider;
use prodex_core::AppPaths;
use prodex_provider_core::ProviderId;
use prodex_state::AppState;
use std::ffi::OsString;
use std::path::Path;

use super::remove_first_codex_config_override_pair;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeResumeSessionSettings {
    pub(super) model: Option<String>,
    pub(super) reasoning_effort: Option<String>,
}

pub(super) fn remove_resume_generated_effort_override(
    codex_args: &mut Vec<OsString>,
    is_resume: bool,
    effort_is_explicit: bool,
) {
    if is_resume && !effort_is_explicit {
        remove_first_codex_config_override_pair(codex_args, "model_reasoning_effort");
    }
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
    if !prodex_runtime_launch::codex_resume_requested(&normalized) {
        return Ok(None);
    }
    let resume_last = codex_resume_last(&normalized);
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let report =
        if let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(&normalized) {
            match prodex_session_store::resolve_session_report_by_id_in_store(
                &paths.shared_codex_root,
                &state,
                session_id,
            ) {
                Ok(report) => Some(report),
                Err(prodex_session_store::SessionResolveError::Missing { .. }) => None,
                Err(prodex_session_store::SessionResolveError::Ambiguous { .. }) => {
                    bail!("resume target is ambiguous; use the full session UUID")
                }
            }
        } else if resume_last {
            let current_dir = (!codex_resume_all(&normalized))
                .then(std::env::current_dir)
                .transpose()?;
            select_resume_last_report(prodex_session_store::collect_session_reports_with_filter(
                &paths.shared_codex_root,
                prodex_session_store::SessionReportFilter {
                    current_dir: current_dir.as_deref(),
                    ..prodex_session_store::SessionReportFilter::default()
                },
                &state,
            )?)
        } else {
            None
        };
    Ok(report)
}

fn select_resume_last_report(
    reports: Vec<prodex_session_store::SessionReport>,
) -> Option<prodex_session_store::SessionReport> {
    reports
        .into_iter()
        .find(|report| !report_path_is_archived(report.path.as_str()))
}

fn codex_resume_all(codex_args: &[OsString]) -> bool {
    let normalized = prodex_runtime_launch::normalize_run_codex_args(codex_args);
    normalized.iter().any(|arg| arg == "--all")
}

fn codex_resume_last(codex_args: &[OsString]) -> bool {
    let normalized = prodex_runtime_launch::normalize_run_codex_args(codex_args);
    normalized.iter().any(|arg| arg == "--last")
}

fn report_path_is_archived(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|component| component.as_os_str() == "archived_sessions")
}

fn resolve_bound_provider_identity(value: Option<&str>) -> Result<Option<ProviderId>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.eq_ignore_ascii_case("amazon-bedrock")
        || value.eq_ignore_ascii_case("amazon-bedrock-runtime")
    {
        return Ok(None);
    }
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
        SuperExternalProvider, codex_resume_last, resolve_bound_provider_identity,
        runtime_external_provider_from_model_provider_id, select_resume_last_report,
    };
    use std::ffi::OsString;

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

    #[test]
    fn upstream_bedrock_provider_ids_remain_direct_on_resume() {
        for provider in ["amazon-bedrock", "amazon-bedrock-runtime"] {
            assert_eq!(
                resolve_bound_provider_identity(Some(provider)).unwrap(),
                None
            );
        }
    }

    #[test]
    fn resume_last_uses_newest_active_report_not_archived_report() {
        let active = prodex_session_store::SessionReport::from_path(
            std::path::Path::new("/home/test-user/codex/sessions/active.jsonl"),
            2,
        );
        let archived = prodex_session_store::SessionReport::from_path(
            std::path::Path::new("/home/test-user/codex/archived_sessions/archived.jsonl"),
            3,
        );

        assert_eq!(
            select_resume_last_report(vec![archived, active]).map(|report| report.path),
            Some("/home/test-user/codex/sessions/active.jsonl".to_string())
        );
    }

    #[test]
    fn bare_resume_does_not_restore_last_session_settings() {
        assert!(!codex_resume_last(&[OsString::from("resume")]));
        assert!(codex_resume_last(&[
            OsString::from("resume"),
            OsString::from("--last"),
        ]));
    }
}
