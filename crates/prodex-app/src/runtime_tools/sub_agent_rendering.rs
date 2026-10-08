use super::{
    ChildLaunchSpec, ResolvedSuperSubAgent, SUB_AGENT_RECURSION_MARKER, sub_agent_target_label,
};
use prodex_cli::SubAgentReasoningEffort;
use std::ffi::OsString;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

pub(super) const SUB_AGENTS_FILE: &str = "SUB_AGENTS.md";
pub(super) const SUB_AGENT_BLOCK_BEGIN: &str = "<!-- PRODEX SUB-AGENT BEGIN -->";
pub(super) const SUB_AGENT_BLOCK_END: &str = "<!-- PRODEX SUB-AGENT END -->";

#[cfg(test)]
fn render_sub_agent_overlay(sub_agent: &ResolvedSuperSubAgent) -> String {
    let task_dir = PathBuf::from(super::SUB_AGENT_TASK_DIR);
    let spec = ChildLaunchSpec {
        executable: PathBuf::from("prodex"),
        provider: sub_agent.provider,
        model: sub_agent.model.clone(),
        effort: sub_agent.effort,
        local_url: sub_agent.url.clone(),
        presidio_enabled: sub_agent.presidio_enabled,
        required_tools: sub_agent
            .required_tools
            .iter()
            .map(ToString::to_string)
            .collect(),
        max_concurrency: sub_agent.max_concurrency,
        slot_dir: PathBuf::from(super::SUB_AGENT_SLOT_DIR),
        task_dir,
        task_max_bytes: super::SUB_AGENT_TASK_MAX_BYTES,
        recursion_marker: SUB_AGENT_RECURSION_MARKER.to_string(),
    };
    render_sub_agent_overlay_for_spec(sub_agent, &spec, Path::new(super::SUB_AGENT_CONFIG_FILE))
        .expect("Mojo sub-agent overlay renderer returned invalid output")
}

pub(super) fn render_sub_agent_overlay_for_spec(
    sub_agent: &ResolvedSuperSubAgent,
    spec: &ChildLaunchSpec,
    config_path: &Path,
) -> anyhow::Result<String> {
    let effort = sub_agent
        .effort
        .map(SubAgentReasoningEffort::as_str)
        .unwrap_or("provider/model default");
    let task_path = spec.task_dir.join("task-001.txt");
    let model = sub_agent
        .model
        .as_deref()
        .map(redaction::redaction_redact_secret_like_text)
        .unwrap_or_else(|| "provider default".to_string());
    let task_directory = spec.task_dir.display().to_string();
    prodex_mojo_core::sub_agent_policy::render_overlay(
        &prodex_mojo_core::sub_agent_policy::SubAgentOverlayRender {
            provider: sub_agent.provider.label(),
            model: &model,
            reasoning_effort: effort,
            max_concurrency: sub_agent.max_concurrency.get(),
            concurrency_source: sub_agent.max_concurrency.source().label(),
            presidio_enabled: sub_agent.presidio_enabled,
            task_directory: &task_directory,
            task_max_bytes: spec.task_max_bytes,
            recursion_marker: SUB_AGENT_RECURSION_MARKER,
            executable: &spec.executable.display().to_string(),
            config: &config_path.display().to_string(),
            task: &task_path.display().to_string(),
            powershell: cfg!(windows),
        },
    )
    .map_err(|error| anyhow::anyhow!("Mojo sub-agent overlay render failed: {error:?}"))
}

pub(crate) fn render_sub_agent_dry_run_report(
    sub_agent: &ResolvedSuperSubAgent,
) -> anyhow::Result<String> {
    let effort = sub_agent
        .effort
        .map(SubAgentReasoningEffort::as_str)
        .unwrap_or("provider/model default");
    let redacted_model = sub_agent
        .model
        .as_deref()
        .map(redaction::redaction_redact_secret_like_text)
        .unwrap_or_else(|| "provider default".into());
    let required_tools = if sub_agent.required_tools.is_empty() {
        "none".to_string()
    } else {
        sub_agent
            .required_tools
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let launch_target = sub_agent_target_label(&sub_agent.target);
    prodex_mojo_core::sub_agent_policy::render_enabled_dry_run_report(
        &prodex_mojo_core::sub_agent_policy::SubAgentDryRunRender {
            provider: sub_agent.provider.label(),
            model: &redacted_model,
            reasoning_effort: effort,
            max_concurrency: sub_agent.max_concurrency.get(),
            concurrency_source: sub_agent.max_concurrency.source().label(),
            hard_max_concurrency: prodex_cli::HARD_MAX_SUB_AGENT_CONCURRENCY,
            presidio_enabled: sub_agent.presidio_enabled,
            required_tools: &required_tools,
            local_url_present: sub_agent.url.is_some(),
            launch_target,
            recursion_disabled: sub_agent.recursion_disabled,
            recursion_marker: SUB_AGENT_RECURSION_MARKER,
            overlay_file: SUB_AGENTS_FILE,
        },
    )
    .map_err(|error| anyhow::anyhow!("Mojo sub-agent dry-run render failed: {error:?}"))
}

pub(crate) fn render_sub_agent_disabled_dry_run_report(
    presidio_enabled: bool,
) -> anyhow::Result<String> {
    prodex_mojo_core::sub_agent_policy::render_disabled_dry_run_report(presidio_enabled).map_err(
        |error| anyhow::anyhow!("Mojo disabled sub-agent dry-run render failed: {error:?}"),
    )
}

pub(crate) fn redact_super_session_args(args: &[OsString]) -> Vec<OsString> {
    args.iter()
        .map(|arg| redact_super_session_arg(arg).unwrap_or_else(|| arg.clone()))
        .collect()
}

fn redact_super_session_arg(arg: &OsString) -> Option<OsString> {
    let value = arg.to_str()?;
    let redacted = prodex_mojo_core::sub_agent_policy::redact_session_argument(value)
        .expect("Mojo sub-agent session redactor returned invalid output");
    (redacted != value).then(|| OsString::from(redacted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_cli::{SubAgentConfig, SuperLaunchTarget};

    #[test]
    fn overlay_has_bounded_english_rules_and_is_idempotent() {
        let resolved = super::super::resolve_super_sub_agent_config(
            SubAgentConfig::default(),
            SuperLaunchTarget::Fresh,
        )
        .unwrap();
        let first = render_sub_agent_overlay(&resolved);
        let second = render_sub_agent_overlay(&resolved);
        assert_eq!(first, second);
        assert_eq!(
            first
                .lines()
                .filter(|line| {
                    line.as_bytes()
                        .first()
                        .is_some_and(|byte| byte.is_ascii_digit())
                })
                .count(),
            18
        );
        assert!(first.contains("Never have more than 4 child sub-agents active at once."));
        assert!(first.contains("official launcher enforces this limit"));
        assert!(first.contains("accepts only `__sub-agent-exec --config ... --task-file ...`"));
        assert!(first.contains("Presidio is inherited explicitly"));
        assert!(first.contains(
            "Keep integration, testing, and the final response main-owned; never modify the parent profile, base `CODEX_HOME`, or repository `AGENTS.md` to activate delegation."
        ));
        for required in [
            "lead and sole integrator",
            "Plan the decomposition",
            "configured number of child sub-agents",
            "disjoint file ownership",
            "stdout and stderr separately",
            "wait for status",
            "full result",
            "untrusted evidence",
            "main-owned",
            "Retry only after a corrective change",
            "objective completed",
            "files inspected or modified",
            "unresolved risks or recommendations",
        ] {
            assert!(first.contains(required), "missing rule: {required}");
        }
    }

    #[test]
    fn session_arg_redaction_covers_standalone_and_embedded_uuids() {
        let session_id = "00000000-0000-7000-8000-000000000042";
        let redacted = redact_super_session_args(&[
            OsString::from(session_id),
            OsString::from(format!("session_id={session_id}")),
            OsString::from(format!("prefix={session_id};suffix=kept")),
        ]);
        assert_eq!(redacted[0], OsString::from("<SESSION_UUID>"));
        assert_eq!(redacted[1], OsString::from("session_id=<SESSION_UUID>"));
        assert_eq!(
            redacted[2],
            OsString::from("prefix=<SESSION_UUID>;suffix=kept")
        );
    }

    #[test]
    fn overlay_redacts_secret_like_model_and_parent_target() {
        let session_id = "00000000-0000-7000-8000-000000000042";
        let resolved = super::super::resolve_super_sub_agent_config(
            SubAgentConfig {
                model: Some("sk-proj-parent-secret".to_string()),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Resume {
                session_id: session_id.to_string(),
            },
        )
        .unwrap();
        let overlay = render_sub_agent_overlay(&resolved);
        assert!(!overlay.contains(session_id));
        assert!(!overlay.contains("sk-proj-parent-secret"));
        assert!(overlay.contains("never forward the parent UUID, `resume`"));
        assert!(overlay.contains("official launcher"), "{overlay}");
    }

    #[test]
    fn enabled_dry_run_report_is_exact_after_redaction() {
        let session_id = "00000000-0000-7000-8000-000000000042";
        let url = "http://127.0.0.1:11434/v1";
        let model = "sk-proj-sub-agent-secret";
        let resolved = super::super::resolve_super_sub_agent_config(
            SubAgentConfig {
                provider: prodex_provider_core::ProviderId::Local,
                model: Some(model.to_string()),
                url: Some(url.to_string()),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Resume {
                session_id: session_id.to_string(),
            },
        )
        .unwrap();
        let report = render_sub_agent_dry_run_report(&resolved).unwrap();
        assert_eq!(
            report,
            format!(
                "Sub-agent: enabled\nSub-agent provider: {}\nSub-agent model: {}\nSub-agent reasoning effort: provider/model default\nMaximum active sub-agents: 4 (Prodex default)\nSub-agent concurrency hard maximum: 64\nSub-agent concurrency enforcement: cross-process exclusive slot leases\nSub-agent inherited Presidio: disabled\nSub-agent inherited required tools: none\nSub-agent local URL: configured\nSub-agent launch target: resume <SESSION_UUID> (parent resume id is not inherited by children)\nSub-agent recursion disabled: yes\nSub-agent recursion marker: PRODEX_SUB_AGENT=1\nSub-agent child launcher: shell-free internal command\nSub-agent overlay: SUB_AGENTS.md (temporary; full instructions injected into the effective AGENTS file)\n",
                resolved.provider.label(),
                redaction::redaction_redact_secret_like_text(model),
            )
        );
        assert!(!report.contains(url));
        assert!(!report.contains(model));
        assert!(!report.contains(session_id));
    }

    #[test]
    fn disabled_dry_run_reports_no_overlay_or_local_url() {
        let report = render_sub_agent_disabled_dry_run_report(false).unwrap();
        assert_eq!(
            report,
            "Sub-agent: disabled\nSub-agent inherited Presidio: disabled\nSub-agent local URL: absent\nSub-agent recursion disabled: yes\nSub-agent overlay: absent\n"
        );
    }

    #[test]
    fn overlay_render_propagates_mojo_scalar_rejection() {
        let resolved = super::super::resolve_super_sub_agent_config(
            SubAgentConfig::default(),
            SuperLaunchTarget::Fresh,
        )
        .unwrap();
        let mut spec = ChildLaunchSpec {
            executable: PathBuf::from("prodex"),
            provider: resolved.provider,
            model: resolved.model.clone(),
            effort: resolved.effort,
            local_url: resolved.url.clone(),
            presidio_enabled: resolved.presidio_enabled,
            required_tools: vec![],
            max_concurrency: resolved.max_concurrency,
            slot_dir: PathBuf::from(super::super::SUB_AGENT_SLOT_DIR),
            task_dir: PathBuf::from(super::super::SUB_AGENT_TASK_DIR),
            task_max_bytes: super::super::SUB_AGENT_TASK_MAX_BYTES,
            recursion_marker: SUB_AGENT_RECURSION_MARKER.to_string(),
        };
        spec.task_max_bytes = 0;
        let error = render_sub_agent_overlay_for_spec(
            &resolved,
            &spec,
            Path::new(super::super::SUB_AGENT_CONFIG_FILE),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Mojo sub-agent overlay render failed: InvalidInput"
        );
    }
}
