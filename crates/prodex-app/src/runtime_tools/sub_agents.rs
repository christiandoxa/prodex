use anyhow::{Context, Result, bail};
use fs2::FileExt;
use prodex_cli::{
    SubAgentLaunchTarget, SubAgentMaxConcurrency, SubAgentReasoningEffort, SuperLaunchTarget,
};
use prodex_mojo_core::sub_agent_policy::{
    ChildArgvAction, ChildOutcomeAction, ChildSpecScalarViolation, LaunchTargetPlan,
    ProviderUrlViolation, RecursionDecision, SlotLockErrorAction, SlotPlanStep, child_argv_plan,
    child_outcome, child_spec_scalar_violation, launch_target_plan, model_nonempty,
    provider_url_violation, recursion_decision, slot_lock_error_action, slot_plan_step,
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
    let session_id = prodex_runtime_launch::codex_resume_session_id(&normalized);
    let target = launch_target_plan(
        session_id.is_some(),
        prodex_runtime_launch::is_codex_exec_invocation(&normalized),
    )
    .expect("Mojo sub-agent launch target policy returned invalid output");
    match target {
        LaunchTargetPlan::Fresh => SuperLaunchTarget::Fresh,
        LaunchTargetPlan::Exec => SuperLaunchTarget::Exec,
        LaunchTargetPlan::Resume => SuperLaunchTarget::Resume {
            session_id: session_id
                .expect("Mojo resume launch target requires a normalized session")
                .to_owned(),
        },
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
    let config = read_bounded_utf8(
        &args.config,
        SUB_AGENT_TASK_MAX_BYTES,
        "sub-agent launcher config",
    )?;
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
#[path = "sub_agents/tests.rs"]
mod tests;
