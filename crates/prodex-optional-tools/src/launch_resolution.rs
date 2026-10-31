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
        OptionalToolId::Rtk => fast_command_tool_status(id, "rtk"),
        OptionalToolId::CodebaseMemoryMcp => fast_command_tool_status(id, "codebase-memory-mcp"),
        OptionalToolId::PlaywrightMcp => playwright_tool_status(),
        OptionalToolId::Ponytail => ponytail_tool_status(),
        OptionalToolId::Presidio => optional_tool_status(id),
    }
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

fn fast_resolved_managed_command(
    id: OptionalToolId,
    root: &Path,
    path: PathBuf,
) -> Result<ResolvedTool> {
    let program = validated_managed_tool_file(root, &path)?;
    resolved_command_tool_for_launch(id, program, ToolDiscoverySource::ManagedRoot)
}

fn fast_resolved_command(
    id: OptionalToolId,
    path: PathBuf,
    source: ToolDiscoverySource,
) -> Result<ResolvedTool> {
    resolved_command_tool_for_launch(id, path, source)
}

#[cfg(all(test, unix))]
mod managed_probe_tests {
    use super::*;
    use std::os::unix::{fs::PermissionsExt, fs::symlink};

    #[test]
    fn managed_path_escape_is_rejected_before_any_optional_or_required_probe() {
        let root = crate::tests::temp_dir("managed-preflight");
        let managed = root.join("managed");
        fs::create_dir_all(&managed).unwrap();
        let outside = root.join("outside");
        fs::write(
            &outside,
            "#!/bin/sh\n: > \"$0.executed\"\nprintf 'rtk 1.1.0\\n'\n",
        )
        .unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o700)).unwrap();
        let candidate = managed.join("rtk");
        symlink(&outside, &candidate).unwrap();
        let marker = root.join("outside.executed");
        let required = resolved_managed_command(
            OptionalToolId::Rtk,
            &managed,
            candidate.clone(),
            &["--version"],
        );
        let optional = fast_resolved_managed_command(OptionalToolId::Rtk, &managed, candidate);
        let executed = marker.exists();
        fs::remove_dir_all(root).unwrap();
        assert!(required.unwrap_err().to_string().contains("escapes"));
        assert!(optional.unwrap_err().to_string().contains("escapes"));
        assert!(
            !executed,
            "a rejected managed path must never execute a probe"
        );
    }

    #[test]
    fn playwright_launch_checks_node_before_package_probe() {
        const CHILD: &str = "PRODEX_TEST_PLAYWRIGHT_EXPECTED";
        if let Ok(expected) = std::env::var(CHILD) {
            let selected = [OptionalToolId::PlaywrightMcp].into_iter().collect();
            let plan = resolve_optional_tools_for_launch(&selected, &OptionalToolSet::default());
            assert_eq!(plan.activations.len(), usize::from(expected == "ready"));
            if expected != "ready" {
                assert_eq!(plan.unavailable.len(), 1);
                assert!(plan.unavailable[0].detail.contains("Node.js 18+"));
            }
            return;
        }
        let root = crate::tests::temp_dir("playwright-node-preflight");
        fs::create_dir_all(&root).unwrap();
        let node = root.join("node");
        let npx = root.join("npx");
        fs::write(
            &npx,
            "#!/bin/sh\n: > \"$0.executed\"\nprintf 'Version 0.0.83\\n'\n",
        )
        .unwrap();
        fs::set_permissions(&npx, fs::Permissions::from_mode(0o700)).unwrap();
        let test = format!(
            "{}::playwright_launch_checks_node_before_package_probe",
            module_path!()
        );
        let test = test.split_once("::").unwrap().1;
        let mut outcomes = Vec::new();
        for (version, expected) in [("v17.9.1", "unavailable"), ("v18.0.0", "ready")] {
            fs::write(&node, format!("#!/bin/sh\nprintf '{version}\\n'\n")).unwrap();
            fs::set_permissions(&node, fs::Permissions::from_mode(0o700)).unwrap();
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", test, "--test-threads=1"])
                .env(CHILD, expected)
                .env("PATH", &root)
                .env("HOME", &root)
                .env("USERPROFILE", &root)
                .env("XDG_DATA_HOME", &root)
                .env(crate::PRODEX_OPTIMIZERS_HOME_ENV, &root)
                .output()
                .unwrap();
            let probed = root.join("npx.executed").exists();
            if probed {
                fs::remove_file(root.join("npx.executed")).unwrap();
            }
            outcomes.push((result, probed, expected));
        }
        fs::remove_dir_all(root).unwrap();
        for (result, probed, expected) in outcomes {
            assert!(
                result.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(probed, expected == "ready");
        }
    }
}
