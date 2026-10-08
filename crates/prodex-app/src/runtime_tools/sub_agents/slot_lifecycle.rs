//! Sub-agent slot acquisition, resizing, and launch-boundary validation.
//! Production state decisions are delegated exclusively to Mojo.

use super::*;

pub(super) fn reconcile_sub_agent_slots(slot_dir: &Path, limit: u16) -> Result<()> {
    let mut stale = Vec::new();
    let mut cursor = 0;
    let mut stale_removed = false;
    loop {
        let step = slot_plan_step(limit, cursor, true)
            .map_err(|error| anyhow::anyhow!("Mojo sub-agent slot planner failed: {error:?}"))?;
        match step {
            SlotPlanStep::Retire { index } => {
                if let Some(locked) = lock_stale_sub_agent_slot(slot_dir, index)? {
                    stale.push(locked);
                }
            }
            SlotPlanStep::Ensure { index } => {
                if !stale_removed {
                    remove_stale_sub_agent_slots(&stale)?;
                    stale_removed = true;
                }
                ensure_sub_agent_slot(slot_dir, index)?;
            }
            SlotPlanStep::Complete => break,
            SlotPlanStep::Candidate { .. } | SlotPlanStep::LimitReached { .. } => {
                bail!("Mojo returned an admission step while reconciling slots");
            }
        }
        cursor += 1;
    }
    Ok(())
}

fn lock_stale_sub_agent_slot(slot_dir: &Path, index: u16) -> Result<Option<(PathBuf, File)>> {
    let slot = slot_dir.join(sub_agent_slot_name(index)?);
    let file = match OpenOptions::new().read(true).write(true).open(&slot) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to open stale concurrency slot {index}"));
        }
    };
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some((slot, file))),
        Err(error) => match sub_agent_slot_lock_error_action(&error, true)? {
            SlotLockErrorAction::BlockResize => bail!(
                "cannot reduce sub-agent concurrency while a child holds slot {index}; wait for active children to finish"
            ),
            SlotLockErrorAction::Propagate => {
                Err(error).context("failed to inspect stale sub-agent concurrency slot")
            }
            SlotLockErrorAction::TryNext => {
                bail!("Mojo returned an admission action while resizing slots")
            }
        },
    }
}

fn remove_stale_sub_agent_slots(stale: &[(PathBuf, File)]) -> Result<()> {
    for (slot, _) in stale {
        fs::remove_file(slot).with_context(|| {
            format!("failed to remove stale concurrency slot {}", slot.display())
        })?;
    }
    Ok(())
}

fn ensure_sub_agent_slot(slot_dir: &Path, index: u16) -> Result<()> {
    let slot = slot_dir.join(sub_agent_slot_name(index)?);
    match OpenOptions::new().write(true).create_new(true).open(&slot) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to create {}", slot.display())),
    }
}

pub(super) fn create_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("failed to secure {}", path.display()))?;
    }
    Ok(())
}

pub(super) fn validate_child_launch_spec(spec: &ChildLaunchSpec) -> Result<()> {
    if !spec.executable.is_absolute() {
        bail!("sub-agent executable path must be absolute");
    }
    match child_spec_scalar_violation(&spec.recursion_marker, spec.task_max_bytes)
        .expect("Mojo sub-agent child-spec scalar policy returned invalid output")
    {
        Some(ChildSpecScalarViolation::InvalidRecursionMarker) => {
            bail!("sub-agent recursion marker is invalid");
        }
        Some(ChildSpecScalarViolation::InvalidTaskSize) => {
            bail!("sub-agent task size policy is invalid");
        }
        None => {}
    }
    match provider_url_violation(spec.provider == ProviderId::Local, spec.local_url.is_some())
        .expect("Mojo sub-agent provider/URL policy returned invalid output")
    {
        Some(ProviderUrlViolation::LocalRequiresUrl) => {
            bail!("local child provider requires a URL");
        }
        Some(ProviderUrlViolation::NonLocalRejectsUrl) => {
            bail!("child local URL is valid only for the local provider");
        }
        None => {}
    }
    if let Some(url) = spec.local_url.as_deref() {
        prodex_cli::parse_sub_agent_url(url).map_err(anyhow::Error::msg)?;
    }
    for tool in &spec.required_tools {
        tool.parse::<prodex_optional_tools::OptionalToolId>()
            .map_err(|error| anyhow::anyhow!("invalid required optional tool {tool}: {error}"))?;
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct SubAgentSlotLease(File);

impl Drop for SubAgentSlotLease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

pub(super) fn sub_agent_slot_name(index: u16) -> Result<String> {
    prodex_mojo_core::sub_agent_policy::render_slot_lock_name(index)
        .map_err(|error| anyhow::anyhow!("Mojo sub-agent slot name formatter failed: {error:?}"))
}

pub(super) fn sub_agent_slot_lock_error_action(
    error: &io::Error,
    reconcile: bool,
) -> Result<SlotLockErrorAction> {
    let expected = fs2::lock_contended_error().raw_os_error();
    let raw_code_matches = error
        .raw_os_error()
        .zip(expected)
        .is_some_and(|(actual, expected)| actual == expected);
    slot_lock_error_action(
        reconcile,
        error.kind() == io::ErrorKind::WouldBlock,
        raw_code_matches,
    )
    .map_err(|error| anyhow::anyhow!("Mojo sub-agent lock classifier failed: {error:?}"))
}

pub(super) fn acquire_sub_agent_slot(spec: &ChildLaunchSpec) -> Result<SubAgentSlotLease> {
    let mut cursor = 0;
    loop {
        let step = slot_plan_step(spec.max_concurrency.get(), cursor, false)
            .map_err(|error| anyhow::anyhow!("Mojo sub-agent slot planner failed: {error:?}"))?;
        match step {
            SlotPlanStep::Candidate { index } => {
                let path = spec.slot_dir.join(sub_agent_slot_name(index)?);
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .with_context(|| format!("failed to open concurrency slot {index}"))?;
                match file.try_lock_exclusive() {
                    Ok(()) => return Ok(SubAgentSlotLease(file)),
                    Err(error) => match sub_agent_slot_lock_error_action(&error, false)? {
                        SlotLockErrorAction::TryNext => {}
                        SlotLockErrorAction::Propagate => {
                            return Err(error)
                                .context("failed to acquire sub-agent concurrency slot");
                        }
                        SlotLockErrorAction::BlockResize => {
                            bail!("Mojo returned a resize action while acquiring a slot");
                        }
                    },
                }
                cursor += 1;
            }
            SlotPlanStep::LimitReached { exit_code } => {
                return Err(crate::command_dispatch::command_exit_error(
                    exit_code,
                    "sub-agent concurrency limit reached; wait for an active child to finish before retrying",
                ));
            }
            SlotPlanStep::Retire { .. } | SlotPlanStep::Ensure { .. } | SlotPlanStep::Complete => {
                bail!("Mojo returned a reconciliation step while acquiring a slot");
            }
        }
    }
}
