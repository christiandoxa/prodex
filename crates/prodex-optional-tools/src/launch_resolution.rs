use super::*;

/// Resolves optional tools for an interactive launch without running optional health probes.
///
/// Required tools still use the full health path. Optional command tools are accepted only
/// after bounded path/root validation; `capability super-doctor` remains the explicit health
/// check for version, daemon, and package validation.
pub fn resolve_optional_tools_for_launch(
    selected: &OptionalToolSet,
    required: &OptionalToolSet,
) -> ToolActivationPlan {
    let mut plan = ToolActivationPlan::default();
    for id in selected.iter() {
        let started = Instant::now();
        let health = if required.contains(id) {
            optional_tool_status(id)
        } else {
            optional_tool_launch_status(id)
        };
        if health.status == ToolHealthStatus::Installed {
            plan.activations.push(ToolActivation {
                tool: ResolvedTool {
                    descriptor: optional_tool_descriptor(id),
                    source: health.source.unwrap_or(ToolDiscoverySource::ManagedRoot),
                    path: health.path.clone(),
                    version: health.version.clone(),
                    digest: health.digest.clone(),
                },
                required: required.contains(id),
            });
        } else {
            plan.unavailable.push(health);
        }
        emit_optional_tool_timing(id, started);
    }
    plan
}

fn optional_tool_launch_status(id: OptionalToolId) -> ToolHealth {
    match id {
        OptionalToolId::Caveman => fast_caveman_tool_status(),
        OptionalToolId::Rtk => fast_command_tool_status(id, "rtk"),
        OptionalToolId::CodebaseMemoryMcp => fast_command_tool_status(id, "codebase-memory-mcp"),
        OptionalToolId::PlaywrightMcp => fast_playwright_tool_status(),
        OptionalToolId::Ponytail => ponytail_tool_status(),
        OptionalToolId::Presidio => optional_tool_status(id),
    }
}

fn fast_caveman_tool_status() -> ToolHealth {
    crate::resolve_caveman()
        .map(ToolHealth::installed)
        .unwrap_or_else(|_| crate::caveman::caveman_tool_status())
}

fn fast_command_tool_status(id: OptionalToolId, command: &str) -> ToolHealth {
    for root in managed_optimizer_roots() {
        for candidate in managed_optimizer_command_candidates(&root, command) {
            if candidate.is_file() {
                return fast_resolved_managed_command(id, &root, candidate)
                    .map(ToolHealth::installed)
                    .unwrap_or_else(|error| invalid_tool(id, error));
            }
        }
    }
    for root in path_dirs_from_env() {
        let candidate = root.join(command);
        if candidate.is_file() {
            return fast_resolved_command(id, candidate, ToolDiscoverySource::Path)
                .map(ToolHealth::installed)
                .unwrap_or_else(|error| invalid_tool(id, error));
        }
        #[cfg(windows)]
        for suffix in ["exe", "cmd", "bat"] {
            let candidate = root.join(format!("{command}.{suffix}"));
            if candidate.is_file() {
                return fast_resolved_command(id, candidate, ToolDiscoverySource::Path)
                    .map(ToolHealth::installed)
                    .unwrap_or_else(|error| invalid_tool(id, error));
            }
        }
    }
    ToolHealth::missing(
        id,
        format!("{command} was not found in managed roots or PATH"),
    )
}

fn fast_playwright_tool_status() -> ToolHealth {
    let id = OptionalToolId::PlaywrightMcp;
    if find_path_command("node").is_none() {
        return ToolHealth::missing(id, "Node.js 18+ was not found on PATH");
    }
    let Some(npx) = find_path_command("npx") else {
        return ToolHealth::missing(id, "npx was not found on PATH");
    };
    fast_resolved_command(id, npx, ToolDiscoverySource::Path)
        .map(ToolHealth::installed)
        .unwrap_or_else(|error| invalid_tool(id, error))
}

fn fast_resolved_managed_command(
    id: OptionalToolId,
    root: &Path,
    path: PathBuf,
) -> Result<ResolvedTool> {
    let root_metadata = fs::symlink_metadata(root)
        .with_context(|| format!("failed to inspect {}", root.display()))?;
    anyhow::ensure!(
        root_metadata.is_dir() && !root_metadata.file_type().is_symlink(),
        "optional-tool root {} must be a real directory",
        root.display()
    );
    let root = root.canonicalize()?;
    let tool = fast_resolved_command(id, path, ToolDiscoverySource::ManagedRoot)?;
    anyhow::ensure!(
        tool.path
            .as_deref()
            .is_some_and(|path| path.starts_with(&root)),
        "managed optional-tool command escapes {}",
        root.display()
    );
    Ok(tool)
}

fn fast_resolved_command(
    id: OptionalToolId,
    path: PathBuf,
    source: ToolDiscoverySource,
) -> Result<ResolvedTool> {
    let path = validated_tool_file(&path)?;
    Ok(ResolvedTool {
        descriptor: optional_tool_descriptor(id),
        source,
        version: None,
        digest: None,
        path: Some(path),
    })
}
