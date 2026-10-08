use anyhow::{Context, Result, bail};
use fs2::FileExt;
use prodex_cli::{
    SubAgentConfig, SubAgentLaunchTarget, SubAgentMaxConcurrency, SubAgentReasoningEffort,
    SuperLaunchTarget,
};
use prodex_mojo_core::sub_agent_policy::{
    ChildArgvAction, ChildOutcomeAction, ChildSpecScalarViolation, ProviderUrlViolation,
    RecursionDecision, SlotLockErrorAction, SlotPlanStep, child_argv_plan, child_outcome,
    child_spec_scalar_violation, model_nonempty, provider_url_violation, recursion_decision,
    slot_lock_error_action, slot_plan_step,
};
use prodex_provider_core::ProviderId;
use prodex_runtime_launch::ChildProcessPlan;
use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[path = "sub_agent_process.rs"]
mod process;
use process::*;
#[path = "sub_agents/slot_lifecycle.rs"]
mod slot_lifecycle;
use slot_lifecycle::{
    acquire_sub_agent_slot, create_private_directory, reconcile_sub_agent_slots,
    validate_child_launch_spec,
};
#[path = "sub_agents/config.rs"]
mod config;
pub(crate) use config::resolve_super_sub_agent_config;
#[path = "sub_agent_catalog.rs"]
mod catalog;
pub(crate) use catalog::*;
#[path = "sub_agent_rendering.rs"]
mod rendering;
use rendering::{
    SUB_AGENT_BLOCK_BEGIN, SUB_AGENT_BLOCK_END, SUB_AGENTS_FILE, render_sub_agent_overlay_for_spec,
};
pub(crate) use rendering::{
    redact_super_session_args, render_sub_agent_disabled_dry_run_report,
    render_sub_agent_dry_run_report,
};

pub(crate) const SUB_AGENT_RECURSION_MARKER: &str = "PRODEX_SUB_AGENT";
pub(crate) const SUB_AGENT_LAUNCHER_MARKER: &str = "PRODEX_SUB_AGENT_LAUNCHER";
const SUB_AGENT_CONFIG_FILE: &str = "sub-agent-launch.json";
const SUB_AGENT_TASK_DIR: &str = "sub-agent-tasks";
const SUB_AGENT_SLOT_DIR: &str = "sub-agent-slots";
const SUB_AGENT_TASK_MAX_BYTES: usize = 65_536;
const SUB_AGENT_OUTPUT_DRAIN_TIMEOUT: Duration =
    Duration::from_millis(if cfg!(test) { 100 } else { 5_000 });
const SUB_AGENT_CHILD_REAP_TIMEOUT: Duration =
    Duration::from_millis(if cfg!(test) { 250 } else { 5_000 });

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct ChildLaunchSpec {
    executable: PathBuf,
    provider: ProviderId,
    model: Option<String>,
    effort: Option<SubAgentReasoningEffort>,
    local_url: Option<String>,
    presidio_enabled: bool,
    #[serde(default)]
    required_tools: Vec<String>,
    max_concurrency: SubAgentMaxConcurrency,
    slot_dir: PathBuf,
    task_dir: PathBuf,
    task_max_bytes: usize,
    recursion_marker: String,
}

impl std::fmt::Debug for ChildLaunchSpec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChildLaunchSpec")
            .field("executable_resolved", &self.executable.is_absolute())
            .field("provider", &self.provider)
            .field("model_configured", &self.model.is_some())
            .field("effort", &self.effort)
            .field("local_url_configured", &self.local_url.is_some())
            .field("presidio_enabled", &self.presidio_enabled)
            .field("required_tools", &self.required_tools)
            .field("max_concurrency", &self.max_concurrency)
            .field("task_max_bytes", &self.task_max_bytes)
            .field("recursion_marker", &self.recursion_marker)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubAgentRecursionPolicy {
    Allowed,
    Disabled,
}

impl SubAgentRecursionPolicy {
    fn from_decision(decision: RecursionDecision) -> Self {
        match decision {
            RecursionDecision::Allowed | RecursionDecision::InternalLauncher => Self::Allowed,
            RecursionDecision::Disabled => Self::Disabled,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ResolvedSuperSubAgent {
    pub(crate) provider: ProviderId,
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<SubAgentReasoningEffort>,
    pub(crate) url: Option<String>,
    pub(crate) max_concurrency: SubAgentMaxConcurrency,
    pub(crate) target: SubAgentLaunchTarget,
    pub(crate) presidio_enabled: bool,
    pub(crate) required_tools: Vec<prodex_optional_tools::OptionalToolId>,
    pub(crate) recursion_disabled: bool,
}

pub(crate) fn resolve_super_launch_target(codex_args: &[OsString]) -> SuperLaunchTarget {
    let normalized = prodex_runtime_launch::normalize_run_codex_args(codex_args);
    if let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(&normalized) {
        return SuperLaunchTarget::Resume {
            session_id: session_id.to_owned(),
        };
    }
    if prodex_runtime_launch::is_codex_exec_invocation(&normalized) {
        SuperLaunchTarget::Exec
    } else {
        SuperLaunchTarget::Fresh
    }
}

impl std::fmt::Debug for ResolvedSuperSubAgent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResolvedSuperSubAgent")
            .field("provider", &self.provider)
            .field("model_configured", &self.model.is_some())
            .field("effort", &self.effort)
            .field("url_configured", &self.url.is_some())
            .field("max_concurrency", &self.max_concurrency)
            .field("target", &sub_agent_target_label(&self.target))
            .field("presidio_enabled", &self.presidio_enabled)
            .field("required_tools", &self.required_tools)
            .field("recursion_disabled", &self.recursion_disabled)
            .finish()
    }
}

pub(crate) fn sub_agent_recursion_policy() -> SubAgentRecursionPolicy {
    SubAgentRecursionPolicy::from_decision(sub_agent_recursion_decision(
        env::var_os(SUB_AGENT_RECURSION_MARKER).is_some(),
        false,
    ))
}

fn sub_agent_recursion_decision(
    recursion_marker_present: bool,
    launcher_marker_is_one: bool,
) -> RecursionDecision {
    recursion_decision(recursion_marker_present, launcher_marker_is_one)
        .expect("Mojo sub-agent recursion policy returned invalid output")
}

pub(crate) fn write_sub_agent_overlay(
    overlay_home: &Path,
    sub_agent: &ResolvedSuperSubAgent,
) -> Result<PathBuf> {
    let executable = env::current_exe().context("failed to resolve current Prodex executable")?;
    write_sub_agent_overlay_with_executable(overlay_home, sub_agent, executable)
}

fn write_sub_agent_overlay_with_executable(
    overlay_home: &Path,
    sub_agent: &ResolvedSuperSubAgent,
    executable: PathBuf,
) -> Result<PathBuf> {
    let task_dir = overlay_home.join(SUB_AGENT_TASK_DIR);
    let slot_dir = overlay_home.join(SUB_AGENT_SLOT_DIR);
    create_private_directory(&task_dir)?;
    create_private_directory(&slot_dir)?;
    reconcile_sub_agent_slots(&slot_dir, sub_agent.max_concurrency.get())?;
    let spec = ChildLaunchSpec {
        executable,
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
        slot_dir,
        task_dir,
        task_max_bytes: SUB_AGENT_TASK_MAX_BYTES,
        recursion_marker: SUB_AGENT_RECURSION_MARKER.to_string(),
    };
    let config_path = overlay_home.join(SUB_AGENT_CONFIG_FILE);
    let config =
        serde_json::to_vec_pretty(&spec).context("failed to encode sub-agent launcher config")?;
    secret_store::write_private_file_atomic(&config_path, &config)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    let path = overlay_home.join(SUB_AGENTS_FILE);
    let contents = render_sub_agent_overlay_for_spec(sub_agent, &spec, &config_path)?;
    secret_store::write_private_file_atomic(&path, contents.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;
    prodex_optional_tools::upsert_agents_block(
        overlay_home,
        SUB_AGENT_BLOCK_BEGIN,
        SUB_AGENT_BLOCK_END,
        &contents,
    )?;
    Ok(path)
}

pub(crate) fn handle_sub_agent_exec(args: prodex_cli::SubAgentExecArgs) -> Result<()> {
    let recursion = sub_agent_recursion_decision(
        env::var_os(SUB_AGENT_RECURSION_MARKER).is_some(),
        env::var_os(SUB_AGENT_LAUNCHER_MARKER).as_deref() == Some(OsStr::new("1")),
    );
    if recursion == RecursionDecision::Disabled {
        bail!(
            "hidden sub-agent launcher cannot be invoked recursively while {SUB_AGENT_RECURSION_MARKER} is set"
        );
    }
    let config = read_bounded_utf8(&args.config, 65_536, "sub-agent launcher config")?;
    let spec: ChildLaunchSpec =
        serde_json::from_str(&config).context("invalid sub-agent launcher config")?;
    validate_child_launch_spec(&spec)?;
    let task_dir = fs::canonicalize(&spec.task_dir).with_context(|| {
        format!(
            "failed to resolve task directory {}",
            spec.task_dir.display()
        )
    })?;
    let task_path = fs::canonicalize(&args.task_file)
        .with_context(|| format!("failed to resolve task file {}", args.task_file.display()))?;
    if task_path.parent() != Some(task_dir.as_path()) {
        bail!("sub-agent task file must be directly inside the configured task directory");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&task_path, fs::Permissions::from_mode(0o600))
            .context("failed to secure sub-agent task file")?;
    }
    let task = read_bounded_utf8(&task_path, spec.task_max_bytes, "sub-agent task")?;
    if !model_nonempty(&task).expect("Mojo sub-agent task validator returned invalid output") {
        bail!("sub-agent task must be nonempty");
    }
    let _slot = acquire_sub_agent_slot(&spec)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to initialize sub-agent launcher runtime")?;
    let outcome = runtime.block_on(run_child(&spec, &task, &task_path))?;
    finish_sub_agent_child(outcome)
}

fn finish_sub_agent_child(outcome: SubAgentChildOutcome) -> Result<()> {
    let action = child_outcome(
        outcome.cancelled,
        outcome.status.success(),
        outcome.output_incomplete,
        outcome.output_bytes > 0,
    )
    .map_err(|error| anyhow::anyhow!("Mojo sub-agent outcome classifier failed: {error:?}"))?;
    match action {
        ChildOutcomeAction::Success => Ok(()),
        ChildOutcomeAction::Cancelled {
            exit_code,
            output_incomplete,
        } => Err(crate::command_dispatch::command_exit_error(
            exit_code,
            if output_incomplete {
                "sub-agent launcher cancelled; child output was incomplete"
            } else {
                "sub-agent launcher cancelled"
            },
        )),
        ChildOutcomeAction::ChildFailed { output_incomplete } => {
            let code = crate::child_exit_code(&outcome.status);
            Err(crate::command_dispatch::command_exit_error(
                code,
                if output_incomplete {
                    format!(
                        "sub-agent child exited with status {code}; child output was incomplete"
                    )
                } else {
                    format!("sub-agent child exited with status {code}")
                },
            ))
        }
        ChildOutcomeAction::OutputIncomplete => {
            bail!("sub-agent child output collection failed");
        }
        ChildOutcomeAction::NoOutput => {
            bail!("sub-agent child completed without output");
        }
    }
}

fn read_bounded_utf8(path: &Path, max_bytes: usize, label: &str) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("failed to open {label}"))?;
    let mut bytes = Vec::with_capacity(max_bytes.min(8_192));
    file.by_ref()
        .take((max_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {label}"))?;
    if bytes.len() > max_bytes {
        bail!("{label} exceeds the {max_bytes}-byte limit");
    }
    String::from_utf8(bytes).with_context(|| format!("{label} must be valid UTF-8"))
}

fn child_argv(spec: &ChildLaunchSpec, task: &str) -> Vec<OsString> {
    let plan = child_argv_plan(
        spec.provider == ProviderId::OpenAi,
        spec.provider == ProviderId::Local,
        spec.presidio_enabled,
        spec.required_tools.len(),
        spec.model.is_some(),
        spec.effort.is_some(),
    )
    .expect("Mojo sub-agent child argv planner returned invalid output");
    let mut tools = spec.required_tools.iter();
    let mut args = Vec::with_capacity(plan.len().saturating_mul(2));

    for action in plan {
        match action {
            ChildArgvAction::Super => args.push(OsString::from("s")),
            ChildArgvAction::NoSubAgent => args.push(OsString::from("--no-sub-agent")),
            ChildArgvAction::Presidio => args.push(OsString::from("--presidio")),
            ChildArgvAction::NoPresidio => args.push(OsString::from("--no-presidio")),
            ChildArgvAction::RequireTool => {
                args.push(OsString::from("--require-tool"));
                args.push(OsString::from(
                    tools.next().expect("Mojo child argv tool count must match"),
                ));
            }
            ChildArgvAction::OpenAiProvider => {
                args.push(OsString::from("-c"));
                args.push(OsString::from("model_provider=\"openai\""));
            }
            ChildArgvAction::LocalProvider => {
                args.push(OsString::from("--url"));
                args.push(OsString::from(
                    spec.local_url
                        .as_deref()
                        .expect("validated local child must have URL"),
                ));
            }
            ChildArgvAction::NamedProvider => {
                args.push(OsString::from("--provider"));
                args.push(OsString::from(spec.provider.label()));
            }
            ChildArgvAction::Model => {
                args.push(OsString::from("--model"));
                args.push(OsString::from(
                    spec.model
                        .as_deref()
                        .expect("Mojo child argv model action requires model"),
                ));
            }
            ChildArgvAction::Effort => {
                args.push(OsString::from("-c"));
                args.push(OsString::from(format!(
                    "model_reasoning_effort={}",
                    spec.effort
                        .expect("Mojo child argv effort action requires effort")
                        .as_str()
                )));
            }
            ChildArgvAction::Exec => args.push(OsString::from("exec")),
            ChildArgvAction::Task => args.push(OsString::from(task)),
        }
    }
    debug_assert!(tools.next().is_none());
    args
}

pub(crate) fn apply_sub_agent_recursion_marker(
    child: &mut ChildProcessPlan,
    sub_agent: Option<&ResolvedSuperSubAgent>,
) {
    let Some(_sub_agent) = sub_agent else {
        return;
    };
    let key = OsString::from(SUB_AGENT_RECURSION_MARKER);
    if let Some((_, value)) = child.extra_env.iter_mut().find(|(name, _)| name == &key) {
        *value = OsString::from("1");
    } else {
        child.extra_env.push((key, OsString::from("1")));
    }
    let launcher_key = OsString::from(SUB_AGENT_LAUNCHER_MARKER);
    if let Some((_, value)) = child
        .extra_env
        .iter_mut()
        .find(|(name, _)| name == &launcher_key)
    {
        *value = OsString::from("1");
    } else {
        child.extra_env.push((launcher_key, OsString::from("1")));
    }
}

fn sub_agent_target_label(target: &SuperLaunchTarget) -> &'static str {
    target.redacted_label()
}

#[cfg(test)]
mod tests {
    use super::{slot_lifecycle::sub_agent_slot_lock_error_action, *};
    use prodex_cli::SubAgentConcurrencySource;

    #[cfg(unix)]
    #[test]
    fn child_exit_code_preserves_signal_status() {
        use std::os::unix::process::ExitStatusExt;

        assert_eq!(
            crate::child_exit_code(&std::process::ExitStatus::from_raw(9)),
            137
        );
        assert_eq!(
            crate::child_exit_code(&std::process::ExitStatus::from_raw(7 << 8)),
            7
        );
    }

    fn temp_test_root(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "prodex-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn slot_spec(root: &Path, limit: u16) -> ChildLaunchSpec {
        let slot_dir = root.join(SUB_AGENT_SLOT_DIR);
        let task_dir = root.join(SUB_AGENT_TASK_DIR);
        fs::create_dir_all(&slot_dir).unwrap();
        fs::create_dir_all(&task_dir).unwrap();
        for index in 0..limit {
            File::create(slot_dir.join(format!("slot-{index:02}.lock"))).unwrap();
        }
        ChildLaunchSpec {
            executable: env::current_exe().unwrap(),
            provider: ProviderId::OpenAi,
            model: None,
            effort: None,
            local_url: None,
            presidio_enabled: false,
            required_tools: Vec::new(),
            max_concurrency: SubAgentMaxConcurrency::new(limit, SubAgentConcurrencySource::Custom)
                .unwrap(),
            slot_dir,
            task_dir,
            task_max_bytes: SUB_AGENT_TASK_MAX_BYTES,
            recursion_marker: SUB_AGENT_RECURSION_MARKER.to_string(),
        }
    }

    fn exec_args(root: &Path, spec: &ChildLaunchSpec, task: &str) -> prodex_cli::SubAgentExecArgs {
        let config = root.join(SUB_AGENT_CONFIG_FILE);
        let task_file = spec.task_dir.join("task.txt");
        fs::write(&config, serde_json::to_vec(spec).unwrap()).unwrap();
        fs::write(&task_file, task).unwrap();
        prodex_cli::SubAgentExecArgs { config, task_file }
    }

    #[test]
    fn lock_errors_use_mojo_slot_actions() {
        assert_eq!(
            sub_agent_slot_lock_error_action(&fs2::lock_contended_error(), false).unwrap(),
            SlotLockErrorAction::TryNext
        );
        assert_eq!(
            sub_agent_slot_lock_error_action(&fs2::lock_contended_error(), true).unwrap(),
            SlotLockErrorAction::BlockResize
        );
        assert_eq!(
            sub_agent_slot_lock_error_action(
                &io::Error::from(io::ErrorKind::PermissionDenied),
                false,
            )
            .unwrap(),
            SlotLockErrorAction::Propagate
        );
    }

    #[test]
    fn super_launch_target_uses_canonical_normalization_and_resume_detection() {
        const SESSION_ID: &str = "00000000-0000-7000-8000-000000000042";
        let args = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();
        let resume = |session_id: &str| SuperLaunchTarget::Resume {
            session_id: session_id.to_string(),
        };

        assert_eq!(
            resolve_super_launch_target(&args(&["review"])),
            SuperLaunchTarget::Fresh
        );
        assert_eq!(
            resolve_super_launch_target(&args(&["--config", "model=fast", "exec", "review"])),
            SuperLaunchTarget::Exec
        );
        assert_eq!(
            resolve_super_launch_target(&args(&["--config", "model=fast", SESSION_ID])),
            resume(SESSION_ID)
        );
        assert_eq!(
            resolve_super_launch_target(&args(&["resume", "--model", "fast", SESSION_ID])),
            resume(SESSION_ID)
        );
        assert_eq!(
            resolve_super_launch_target(&args(&["exec", "resume", SESSION_ID, "continue"])),
            resume(SESSION_ID)
        );
        assert_eq!(
            resolve_super_launch_target(&args(&["resume", "--last", "continue"])),
            SuperLaunchTarget::Fresh
        );
        assert_eq!(
            resolve_super_launch_target(&args(&["--", SESSION_ID])),
            SuperLaunchTarget::Fresh
        );
    }

    #[test]
    fn default_config_omits_optional_model() {
        let config = SubAgentConfig::default();
        let resolved = resolve_super_sub_agent_config(config, SuperLaunchTarget::Fresh).unwrap();
        assert_eq!(resolved.model, None);
        assert!(resolved.recursion_disabled);
        assert!(canonical_sub_agent_providers().contains(&ProviderId::OpenAi));
        assert!(canonical_sub_agent_model_choices(ProviderId::OpenAi, None).len() > 2);
    }

    #[test]
    fn resolver_rejects_empty_custom_model_at_the_app_boundary() {
        let error = resolve_super_sub_agent_config(
            SubAgentConfig {
                model: Some(" \t".to_string()),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Fresh,
        )
        .unwrap_err();
        assert!(error.to_string().contains("must be nonempty"));
    }

    #[test]
    fn effort_suggestions_fall_back_for_dynamic_models() {
        assert_eq!(
            canonical_sub_agent_efforts(ProviderId::Kiro, Some("account-only-model")),
            canonical_sub_agent_efforts(ProviderId::Kiro, None)
        );
    }

    #[test]
    fn aliases_normalize_and_local_urls_are_typed() {
        let resolved = resolve_super_sub_agent_config(
            SubAgentConfig {
                provider: ProviderId::Local,
                model: Some("default".to_string()),
                model_reasoning_effort: Some(SubAgentReasoningEffort::XHigh),
                url: Some("http://127.0.0.1:11434/v1".to_string()),
                max_concurrency: Default::default(),
            },
            SuperLaunchTarget::Exec,
        )
        .unwrap();
        assert_eq!(resolved.model.as_deref(), Some("local"));
        assert_eq!(resolved.effort, Some(SubAgentReasoningEffort::XHigh));
        assert_eq!(resolved.url.as_deref(), Some("http://127.0.0.1:11434/v1"));
    }

    #[test]
    fn resolver_uses_canonical_mojo_reasoning_compatibility() {
        let error = resolve_super_sub_agent_config(
            SubAgentConfig {
                provider: ProviderId::OpenAi,
                model: Some("gpt-5.6-luna".to_string()),
                model_reasoning_effort: Some(SubAgentReasoningEffort::Ultra),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Fresh,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("reasoning effort ultra is unsupported for openai model gpt-5.6-luna")
        );

        let resolved = resolve_super_sub_agent_config(
            SubAgentConfig {
                provider: ProviderId::OpenAi,
                model: Some("account/model".to_string()),
                model_reasoning_effort: Some(SubAgentReasoningEffort::Ultra),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Fresh,
        )
        .unwrap();
        assert_eq!(resolved.model.as_deref(), Some("account/model"));
        assert_eq!(resolved.effort, Some(SubAgentReasoningEffort::Ultra));
    }

    #[test]
    fn resolver_preserves_mojo_error_precedence_across_facts() {
        let model_error = resolve_super_sub_agent_config(
            SubAgentConfig {
                model: Some(" \t".to_string()),
                url: Some("not-a-url".to_string()),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Fresh,
        )
        .unwrap_err();
        assert!(
            model_error
                .to_string()
                .contains("--sub-agent-model must be nonempty")
        );

        let reasoning_error = resolve_super_sub_agent_config(
            SubAgentConfig {
                model: Some("gpt-5.6-luna".to_string()),
                model_reasoning_effort: Some(SubAgentReasoningEffort::Ultra),
                url: Some("not-a-url".to_string()),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Fresh,
        )
        .unwrap_err();
        assert!(
            reasoning_error
                .to_string()
                .contains("reasoning effort ultra is unsupported")
        );

        let url_error = resolve_super_sub_agent_config(
            SubAgentConfig {
                model: Some("account/model".to_string()),
                url: Some("not-a-url".to_string()),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Fresh,
        )
        .unwrap_err();
        assert!(
            !url_error
                .to_string()
                .contains("--sub-agent-url is only supported with the local sub-agent provider")
        );

        let provider_url_error = resolve_super_sub_agent_config(
            SubAgentConfig {
                model: Some("account/model".to_string()),
                url: Some("http://127.0.0.1:11434/v1".to_string()),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Fresh,
        )
        .unwrap_err();
        assert!(
            provider_url_error
                .to_string()
                .contains("--sub-agent-url is only supported with the local sub-agent provider")
        );
    }

    #[test]
    fn local_provider_requires_endpoint() {
        let error = resolve_super_sub_agent_config(
            SubAgentConfig {
                provider: ProviderId::Local,
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Fresh,
        )
        .unwrap_err();
        assert!(error.to_string().contains("requires --sub-agent-url"));
    }

    fn test_spec(provider: ProviderId) -> ChildLaunchSpec {
        ChildLaunchSpec {
            executable: PathBuf::from("/opt/Prodex Binary/prodex"),
            provider,
            model: None,
            effort: None,
            local_url: None,
            presidio_enabled: false,
            required_tools: Vec::new(),
            max_concurrency: SubAgentMaxConcurrency::default(),
            slot_dir: PathBuf::from("sub-agent-slots"),
            task_dir: PathBuf::from("sub-agent-tasks"),
            task_max_bytes: SUB_AGENT_TASK_MAX_BYTES,
            recursion_marker: SUB_AGENT_RECURSION_MARKER.to_string(),
        }
    }

    #[test]
    fn child_argv_is_shell_free_exact_and_never_inherits_parent_uuid() {
        let task = "spaces 'apostrophe' \"quotes\"\nUnicode 任务; $(touch nope) & |";
        let mut spec = test_spec(ProviderId::Copilot);
        spec.model = Some("模型/β-🦀".to_string());
        spec.effort = Some(SubAgentReasoningEffort::XHigh);
        spec.presidio_enabled = true;
        spec.required_tools = [
            prodex_optional_tools::OptionalToolId::Rtk,
            prodex_optional_tools::OptionalToolId::Ponytail,
        ]
        .into_iter()
        .map(|tool| tool.to_string())
        .collect();
        let args = child_argv(&spec, task);
        assert_eq!(args[0], "s");
        assert_eq!(args[1], "--no-sub-agent");
        assert_eq!(
            args.iter()
                .filter(|value| **value == "--presidio" || **value == "--no-presidio")
                .count(),
            1
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--provider", "copilot"])
        );
        assert_eq!(
            args.windows(2)
                .filter(|pair| pair[0] == "--require-tool")
                .collect::<Vec<_>>(),
            vec![
                [OsString::from("--require-tool"), OsString::from("rtk")],
                [OsString::from("--require-tool"), OsString::from("ponytail")],
            ]
        );
        assert!(args.windows(2).any(|pair| pair == ["--model", "模型/β-🦀"]));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["-c", "model_reasoning_effort=xhigh"])
        );
        assert_eq!(args[args.len() - 2], "exec");
        assert_eq!(args.last().unwrap(), task);
        assert_eq!(args.iter().filter(|value| **value == task).count(), 1);
        assert!(
            !args
                .iter()
                .any(|value| value.to_string_lossy().contains("019c"))
        );
    }

    #[test]
    fn openai_child_argv_uses_accepted_override_and_cannot_inherit_profile_provider() {
        let args = child_argv(&test_spec(ProviderId::OpenAi), "task");
        assert!(!args.iter().any(|arg| arg == "--provider"));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["-c", "model_provider=\"openai\""])
        );
        let parsed = prodex_cli::parse_cli_command_from(
            std::iter::once(OsString::from("prodex")).chain(args),
        )
        .unwrap();
        let prodex_cli::Commands::Super(parsed) = parsed else {
            panic!("child argv must parse as Super");
        };
        assert!(parsed.provider.is_none());
        assert!(
            parsed
                .codex_args
                .windows(2)
                .any(|pair| pair == ["-c", "model_provider=\"openai\""])
        );
    }

    #[test]
    fn local_child_argv_keeps_exact_url() {
        let mut spec = test_spec(ProviderId::Local);
        spec.local_url = Some("http://127.0.0.1:8131/v1".to_string());
        let args = child_argv(&spec, "task");
        assert!(
            args.windows(2)
                .any(|pair| { pair == ["--url", "http://127.0.0.1:8131/v1"] })
        );
        assert!(!args.iter().any(|value| value == "--provider"));
    }

    #[test]
    fn child_config_serializes_required_tools_and_rejects_unknown_names() {
        let root = temp_test_root("sub-agent-required-tools");
        create_private_directory(&root).unwrap();
        let mut resolved =
            resolve_super_sub_agent_config(SubAgentConfig::default(), SuperLaunchTarget::Fresh)
                .unwrap();
        resolved.required_tools = [
            prodex_optional_tools::OptionalToolId::CodebaseMemoryMcp,
            prodex_optional_tools::OptionalToolId::Rtk,
        ]
        .into_iter()
        .collect();

        write_sub_agent_overlay_with_executable(
            &root,
            &resolved,
            env::temp_dir().join("Prodex Binary").join("prodex"),
        )
        .unwrap();
        let config = std::fs::read_to_string(root.join(SUB_AGENT_CONFIG_FILE)).unwrap();
        let spec: ChildLaunchSpec = serde_json::from_str(&config).unwrap();
        assert_eq!(spec.required_tools, vec!["codebase-memory-mcp", "rtk"]);
        validate_child_launch_spec(&spec).unwrap();

        let mut invalid = serde_json::to_value(&spec).unwrap();
        invalid["required-tools"] = serde_json::json!(["not-a-tool"]);
        let invalid: ChildLaunchSpec = serde_json::from_value(invalid).unwrap();
        assert!(validate_child_launch_spec(&invalid).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recursion_marker_and_internal_launcher_use_mojo_policy() {
        assert_eq!(
            sub_agent_recursion_decision(false, false),
            RecursionDecision::Allowed
        );
        assert_eq!(
            sub_agent_recursion_decision(true, false),
            RecursionDecision::Disabled
        );
        assert_eq!(
            sub_agent_recursion_decision(true, true),
            RecursionDecision::InternalLauncher
        );
        assert_eq!(
            SubAgentRecursionPolicy::from_decision(sub_agent_recursion_decision(true, false)),
            SubAgentRecursionPolicy::Disabled
        );
    }

    #[test]
    fn dry_run_redacts_endpoint_and_resume_id() {
        let session_id = "00000000-0000-7000-8000-000000000042";
        let url = "http://127.0.0.1:11434/v1";
        let model = "sk-proj-sub-agent-secret";
        let resolved = resolve_super_sub_agent_config(
            SubAgentConfig {
                provider: ProviderId::Local,
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
        let debug = format!("{resolved:?}");
        assert!(report.contains("Sub-agent local URL: configured"));
        assert!(report.contains("Sub-agent inherited required tools: none"));
        assert!(report.contains("Sub-agent launch target: resume <SESSION_UUID>"));
        assert!(report.contains("Sub-agent recursion disabled: yes"));
        assert!(!report.contains(url));
        assert!(!report.contains(model));
        assert!(!report.contains(session_id));
        assert!(
            debug.contains("target: \"resume <SESSION_UUID>\""),
            "{debug}"
        );
        assert!(!debug.contains(session_id), "{debug}");
    }

    #[test]
    fn overlay_and_child_marker_are_scoped_to_the_resolved_launch() {
        let root = env::temp_dir().join(format!(
            "prodex-sub-agent-overlay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .unwrap();
        let mut resolved = resolve_super_sub_agent_config(
            SubAgentConfig {
                model: Some("gpt-5.4".to_string()),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Resume {
                session_id: "00000000-0000-7000-8000-000000000042".to_string(),
            },
        )
        .unwrap();
        resolved.presidio_enabled = true;

        let path = write_sub_agent_overlay(&root, &resolved).unwrap();
        let contents = std::fs::read_to_string(path).unwrap();
        assert!(contents.contains("--presidio"));
        assert!(!contents.contains("00000000-0000-7000-8000-000000000042"));
        let agents = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains(SUB_AGENT_BLOCK_BEGIN));
        assert!(agents.contains("Never have more than 4 child sub-agents active at once."));
        assert!(!agents.contains("@/") && !agents.contains("@SUB_AGENTS.md"));
        assert_eq!(
            std::fs::read_dir(root.join(SUB_AGENT_SLOT_DIR))
                .unwrap()
                .count(),
            usize::from(resolved.max_concurrency.get())
        );

        let mut child = ChildProcessPlan::new(OsString::from("codex"), root.clone());
        apply_sub_agent_recursion_marker(&mut child, Some(&resolved));
        assert_eq!(
            child
                .extra_env
                .iter()
                .find(|(name, _)| name == SUB_AGENT_RECURSION_MARKER)
                .map(|(_, value)| value.as_os_str()),
            Some(std::ffi::OsStr::new("1"))
        );
        assert!(
            child
                .extra_env
                .iter()
                .any(|(name, value)| name == SUB_AGENT_LAUNCHER_MARKER && value == "1")
        );
        assert_eq!(child.extra_env.len(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn child_config_contains_only_launch_data_and_no_parent_uuid() {
        let model = "sk-proj-synthetic-model-id";
        let session_id = "00000000-0000-7000-8000-000000000042";
        let resolved = resolve_super_sub_agent_config(
            SubAgentConfig {
                model: Some(model.to_string()),
                ..SubAgentConfig::default()
            },
            SuperLaunchTarget::Resume {
                session_id: session_id.to_string(),
            },
        )
        .unwrap();
        let root = env::temp_dir().join(format!(
            "prodex-sub-agent-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        create_private_directory(&root).unwrap();
        write_sub_agent_overlay_with_executable(
            &root,
            &resolved,
            PathBuf::from("/opt/Prodex Binary/prodex"),
        )
        .unwrap();
        let config = std::fs::read_to_string(root.join(SUB_AGENT_CONFIG_FILE)).unwrap();
        assert!(config.contains(model));
        assert!(!config.contains(session_id));
        for forbidden in ["api_key", "oauth", "authorization", "bearer", "cookie"] {
            assert!(!config.to_ascii_lowercase().contains(forbidden), "{config}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn child_config_rejects_unknown_fields() {
        let root = temp_test_root("sub-agent-config-unknown-field");
        let spec = slot_spec(&root, 1);
        let mut config = serde_json::to_value(&spec).unwrap();
        config
            .as_object_mut()
            .unwrap()
            .insert("api-key".to_string(), serde_json::json!("synthetic"));

        assert!(serde_json::from_value::<ChildLaunchSpec>(config).is_err());

        let mut config = serde_json::to_value(&spec).unwrap();
        config["max-concurrency"]
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_string(), serde_json::json!(true));
        assert!(serde_json::from_value::<ChildLaunchSpec>(config).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repeated_setup_reconciles_exact_slots_and_protects_active_downsize() {
        let root = temp_test_root("sub-agent-slot-reconcile");
        create_private_directory(&root).unwrap();
        let mut resolved =
            resolve_super_sub_agent_config(SubAgentConfig::default(), SuperLaunchTarget::Fresh)
                .unwrap();
        resolved.max_concurrency = prodex_cli::parse_sub_agent_max_concurrency("8").unwrap();
        let executable = PathBuf::from("/opt/Prodex Binary/prodex");

        write_sub_agent_overlay_with_executable(&root, &resolved, executable.clone()).unwrap();
        write_sub_agent_overlay_with_executable(&root, &resolved, executable.clone()).unwrap();
        let slot_dir = root.join(SUB_AGENT_SLOT_DIR);
        assert_eq!(fs::read_dir(&slot_dir).unwrap().count(), 8);
        let mut names = fs::read_dir(&slot_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            (0..8)
                .map(|index| format!("slot-{index:02}.lock"))
                .collect::<Vec<_>>()
        );

        let active = OpenOptions::new()
            .read(true)
            .write(true)
            .open(slot_dir.join("slot-07.lock"))
            .unwrap();
        FileExt::lock_exclusive(&active).unwrap();
        resolved.max_concurrency = prodex_cli::parse_sub_agent_max_concurrency("4").unwrap();
        let error = write_sub_agent_overlay_with_executable(&root, &resolved, executable.clone())
            .unwrap_err();
        assert!(
            error.to_string().contains("wait for active children"),
            "{error:#}"
        );
        assert_eq!(fs::read_dir(&slot_dir).unwrap().count(), 8);
        FileExt::unlock(&active).unwrap();
        drop(active);

        write_sub_agent_overlay_with_executable(&root, &resolved, executable.clone()).unwrap();
        assert_eq!(fs::read_dir(&slot_dir).unwrap().count(), 4);
        resolved.max_concurrency = prodex_cli::parse_sub_agent_max_concurrency("16").unwrap();
        write_sub_agent_overlay_with_executable(&root, &resolved, executable).unwrap();
        assert_eq!(fs::read_dir(&slot_dir).unwrap().count(), 16);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&slot_dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(root.join(SUB_AGENT_TASK_DIR))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(root.join(SUB_AGENT_CONFIG_FILE))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn slot_limits_are_bounded_release_and_reusable() {
        for limit in [1, 4, 8, 23, 64] {
            let root = temp_test_root("sub-agent-slot-limit");
            let spec = slot_spec(&root, limit);
            let mut leases = Vec::new();
            let mut maximum_observed_concurrency = 0;
            for _ in 0..limit {
                leases.push(acquire_sub_agent_slot(&spec).unwrap());
                maximum_observed_concurrency = maximum_observed_concurrency.max(leases.len());
            }
            assert!(maximum_observed_concurrency <= usize::from(limit));
            assert_eq!(maximum_observed_concurrency, usize::from(limit));
            assert_eq!(leases.len(), usize::from(limit));
            let error = acquire_sub_agent_slot(&spec).unwrap_err().to_string();
            assert!(
                error.contains("sub-agent concurrency limit reached"),
                "{error}"
            );
            drop(leases.pop());
            leases.push(acquire_sub_agent_slot(&spec).unwrap());
            assert_eq!(leases.len(), usize::from(limit));
            drop(leases);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn failed_spawn_releases_its_cross_process_slot() {
        let root = temp_test_root("sub-agent-failed-spawn");
        let mut spec = slot_spec(&root, 1);
        spec.executable = root.join("missing-prodex-binary");
        let error = handle_sub_agent_exec(exec_args(&root, &spec, "narrow task")).unwrap_err();
        assert!(
            spec.task_dir.join("task.txt").exists()
                && error
                    .to_string()
                    .contains("failed to spawn sub-agent child")
        );
        drop(acquire_sub_agent_slot(&spec).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_task_is_rejected_by_mojo_before_slot_admission() {
        let root = temp_test_root("sub-agent-empty-task");
        let spec = slot_spec(&root, 1);
        let args = exec_args(&root, &spec, " \u{3000}\t");
        let error = handle_sub_agent_exec(args).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("sub-agent task must be nonempty")
        );
        assert!(spec.task_dir.join("task.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn mojo_child_outcome_plan_preserves_failure_precedence() {
        use std::os::unix::process::ExitStatusExt;

        let cancelled = finish_sub_agent_child(SubAgentChildOutcome {
            status: std::process::ExitStatus::from_raw(0),
            cancelled: true,
            output_incomplete: true,
            output_bytes: 0,
        })
        .unwrap_err();
        assert!(cancelled.to_string().contains("launcher cancelled"));
        assert!(cancelled.to_string().contains("output was incomplete"));

        let child_failed = finish_sub_agent_child(SubAgentChildOutcome {
            status: std::process::ExitStatus::from_raw(7 << 8),
            cancelled: false,
            output_incomplete: true,
            output_bytes: 0,
        })
        .unwrap_err();
        assert!(
            child_failed
                .to_string()
                .contains("child exited with status 7")
        );
        assert!(child_failed.to_string().contains("output was incomplete"));

        let no_output = finish_sub_agent_child(SubAgentChildOutcome {
            status: std::process::ExitStatus::from_raw(0),
            cancelled: false,
            output_incomplete: false,
            output_bytes: 0,
        })
        .unwrap_err();
        assert!(no_output.to_string().contains("completed without output"));
    }

    #[test]
    fn inherited_output_pipe_is_bounded_and_cannot_hold_a_slot() {
        let root = temp_test_root("sub-agent-held-output-pipe");
        let spec = slot_spec(&root, 1);
        let started = std::time::Instant::now();
        {
            let _slot = acquire_sub_agent_slot(&spec).unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let error = runtime
                .block_on(async {
                    let (stdout_reader, _held_by_descendant) = tokio::io::duplex(1);
                    let (stderr_reader, stderr_writer) = tokio::io::duplex(1);
                    drop(stderr_writer);
                    drain_child_output_tasks(
                        tokio::spawn(relay_child_output(stdout_reader, tokio::io::sink())),
                        tokio::spawn(relay_child_output(stderr_reader, tokio::io::sink())),
                    )
                    .await
                })
                .unwrap_err();
            assert!(error.to_string().contains("output drain timed out"));
        }
        assert!(started.elapsed() < Duration::from_secs(1));
        drop(acquire_sub_agent_slot(&spec).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn child_process_group_cleanup_reaches_descendants() {
        let root = temp_test_root("sub-agent-process-group");
        fs::create_dir_all(&root).unwrap();
        let pid_file = root.join("descendant.pid");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut command = tokio::process::Command::new("sh");
            command.args([
                "-c",
                "sleep 30 & echo $! > \"$1\"; wait",
                "sh",
                pid_file.to_str().unwrap(),
            ]);
            configure_sub_agent_child_process_group(&mut command);
            let mut child = command.spawn().unwrap();
            let process_group_id = child.id();
            let mut descendant = None;
            for _ in 0..100 {
                descendant = fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|value| value.trim().parse::<libc::pid_t>().ok());
                if descendant.is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let descendant = descendant.expect("descendant pid should be written completely");

            assert_eq!(
                unsafe { libc::getpgid(descendant) },
                process_group_id.unwrap() as libc::pid_t
            );
            terminate_sub_agent_child(&mut child, process_group_id).unwrap();
            child.wait().await.unwrap();
            for _ in 0..100 {
                let absent = unsafe { libc::kill(descendant, 0) } != 0
                    && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
                let zombie = fs::read_to_string(format!("/proc/{descendant}/stat"))
                    .ok()
                    .and_then(|stat| stat.rsplit_once(") ").map(|(_, rest)| rest.to_string()))
                    .and_then(|rest| rest.split_whitespace().next().map(str::to_string))
                    .is_some_and(|state| state == "Z");
                if absent || zombie {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("descendant remained alive after process-group cleanup");
        });
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn child_job_cleanup_reaches_descendants() {
        use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
        };

        let root = temp_test_root("sub-agent-job");
        fs::create_dir_all(&root).unwrap();
        let start_file = root.join("start");
        let pid_file = root.join("descendant.pid");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let descendant = runtime.block_on(async {
            let mut command = tokio::process::Command::new("powershell.exe");
            command
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "while (-not (Test-Path -LiteralPath $env:PRODEX_TEST_START_FILE)) { Start-Sleep -Milliseconds 10 }; $child = Start-Process powershell.exe -ArgumentList @('-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 30') -PassThru; Set-Content -LiteralPath $env:PRODEX_TEST_PID_FILE -Value $child.Id",
                ])
                .env("PRODEX_TEST_START_FILE", &start_file)
                .env("PRODEX_TEST_PID_FILE", &pid_file);
            let mut child = command.spawn().unwrap();
            let job = assign_sub_agent_child_job(&child).unwrap();
            fs::write(&start_file, "start").unwrap();
            assert!(child.wait().await.unwrap().success());
            let descendant = fs::read_to_string(&pid_file)
                .unwrap()
                .trim()
                .parse::<u32>()
                .unwrap();
            drop(job);
            descendant
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, descendant) };
            if handle.is_null() {
                break;
            }
            // SAFETY: successful OpenProcess returns an owned process handle.
            let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
            if unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) } != WAIT_TIMEOUT {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "descendant remained alive"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn limit_reached_secures_and_preserves_task_for_retry() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_test_root("sub-agent-secure-task");
        let spec = slot_spec(&root, 1);
        let lease = acquire_sub_agent_slot(&spec).unwrap();
        let args = exec_args(&root, &spec, "narrow task");
        let task_file = args.task_file.clone();
        fs::set_permissions(&args.task_file, fs::Permissions::from_mode(0o666)).unwrap();

        let error = handle_sub_agent_exec(args).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("sub-agent concurrency limit reached")
        );
        assert!(task_file.exists());
        assert_eq!(
            fs::metadata(&task_file).unwrap().permissions().mode() & 0o777,
            0o600
        );

        drop(lease);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn slot_holder_process() {
        let Some(root) = env::var_os("PRODEX_TEST_SUB_AGENT_SLOT_ROOT") else {
            return;
        };
        let limit = env::var("PRODEX_TEST_SUB_AGENT_SLOT_LIMIT")
            .unwrap()
            .parse::<u16>()
            .unwrap();
        let result_dir = PathBuf::from(&root).join("results");
        fs::create_dir_all(&result_dir).unwrap();
        let spec = slot_spec(Path::new(&root), limit);
        let result = result_dir.join(format!("{}.txt", std::process::id()));
        match acquire_sub_agent_slot(&spec) {
            Ok(_lease) => {
                fs::write(&result, "acquired").unwrap();
                std::thread::sleep(std::time::Duration::from_millis(1_500));
            }
            Err(error) => {
                assert!(
                    error
                        .to_string()
                        .contains("sub-agent concurrency limit reached")
                );
                fs::write(&result, "rejected").unwrap();
            }
        }
    }

    #[test]
    fn separate_processes_share_limit_and_os_releases_stale_slot() {
        let root = temp_test_root("sub-agent-cross-process");
        let spec = slot_spec(&root, 4);
        let test_name = "runtime_tools::sub_agents::tests::slot_holder_process";
        let executable = env::current_exe().unwrap();
        let mut children = (0..5)
            .map(|_| {
                for _ in 0..20 {
                    match std::process::Command::new(&executable)
                        .args(["--exact", test_name, "--nocapture"])
                        .env("PRODEX_TEST_SUB_AGENT_SLOT_ROOT", &root)
                        .env("PRODEX_TEST_SUB_AGENT_SLOT_LIMIT", "4")
                        .spawn()
                    {
                        Ok(child) => return child,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(error) => panic!("failed to spawn slot holder: {error}"),
                    }
                }
                panic!("test executable remained unavailable while spawning slot holder")
            })
            .collect::<Vec<_>>();
        let result_dir = root.join("results");
        for _ in 0..200 {
            if fs::read_dir(&result_dir)
                .map(|entries| entries.count() == 5)
                .unwrap_or(false)
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let results = fs::read_dir(&result_dir)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let pid = path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .parse::<u32>()
                    .unwrap();
                (pid, fs::read_to_string(path).unwrap())
            })
            .collect::<Vec<_>>();
        let maximum_observed_concurrency = results
            .iter()
            .filter(|(_, value)| value == "acquired")
            .count();
        assert!(maximum_observed_concurrency <= 4);
        assert_eq!(maximum_observed_concurrency, 4);
        assert_eq!(
            results
                .iter()
                .filter(|(_, value)| value == "rejected")
                .count(),
            1
        );
        let started = std::time::Instant::now();
        let error = acquire_sub_agent_slot(&spec).unwrap_err().to_string();
        assert!(started.elapsed() < std::time::Duration::from_millis(250));
        assert!(error.contains("sub-agent concurrency limit reached"));

        let acquired_pid = results
            .iter()
            .find_map(|(pid, value)| (value == "acquired").then_some(*pid))
            .unwrap();
        let acquired_index = children
            .iter()
            .position(|child| child.id() == acquired_pid)
            .unwrap();
        children[acquired_index].kill().unwrap();
        children[acquired_index].wait().unwrap();
        let lease = acquire_sub_agent_slot(&spec).unwrap();
        drop(lease);
        for (index, child) in children.iter_mut().enumerate() {
            if index != acquired_index {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
