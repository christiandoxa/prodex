use super::*;
use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_overlay(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("prodex-{name}-{}-{stamp}", std::process::id()))
}

#[test]
fn optional_incompatible_tool_is_informational_unless_required() {
    let invalid_rtk = prodex_optional_tools::ToolHealth {
        id: prodex_optional_tools::OptionalToolId::Rtk,
        status: prodex_optional_tools::ToolHealthStatus::Invalid,
        source: Some(prodex_optional_tools::ToolDiscoverySource::Path),
        path: Some(PathBuf::from("/home/test-user/.local/bin/rtk")),
        version: Some("0.45.0".to_string()),
        digest: None,
        can_activate: false,
        detail: "rtk 0.45.0 is too old; Prodex requires 0.46.0 or newer".to_string(),
    };
    let plan = prodex_optional_tools::ToolActivationPlan {
        activations: Vec::new(),
        unavailable: vec![invalid_rtk],
    };

    let optional_required = prodex_optional_tools::OptionalToolSet::default();
    let decision = runtime_optional_tool_decision(&plan, &optional_required).unwrap();
    assert_eq!(decision, OverlayToolPlanDecision::SkipIncompatible(1));
    let messages = optional_tool_skip_messages(&plan, &decision);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("rtk: skipped for this launch"));
    assert!(messages[0].contains("Update when convenient"));
    assert!(messages[0].contains("minimum supported: 0.46.0"));

    let required = [prodex_optional_tools::OptionalToolId::Rtk]
        .into_iter()
        .collect::<prodex_optional_tools::OptionalToolSet>();
    let decision = runtime_optional_tool_decision(&plan, &required).unwrap();
    assert_eq!(decision, OverlayToolPlanDecision::RequiredUnavailable(0));
    let error = required_optional_tool_error(&plan, 0);
    assert!(error.contains("required optional tool rtk is unavailable"));

    let mixed = prodex_optional_tools::ToolActivationPlan {
        activations: Vec::new(),
        unavailable: vec![
            plan.unavailable[0].clone(),
            prodex_optional_tools::ToolHealth {
                id: prodex_optional_tools::OptionalToolId::CodebaseMemoryMcp,
                status: prodex_optional_tools::ToolHealthStatus::Missing,
                source: None,
                path: None,
                version: None,
                digest: None,
                can_activate: false,
                detail: "codebase-memory-mcp was not found in managed roots or PATH".into(),
            },
        ],
    };
    let required_codebase = [prodex_optional_tools::OptionalToolId::CodebaseMemoryMcp]
        .into_iter()
        .collect::<prodex_optional_tools::OptionalToolSet>();
    let decision = runtime_optional_tool_decision(&mixed, &required_codebase).unwrap();
    assert_eq!(decision, OverlayToolPlanDecision::RequiredUnavailable(1));
    assert!(optional_tool_skip_messages(&mixed, &decision).is_empty());
}

#[test]
fn optional_tool_plan_without_unavailable_items_is_ready() {
    let plan = prodex_optional_tools::ToolActivationPlan::default();
    assert_eq!(
        runtime_optional_tool_decision(&plan, &prodex_optional_tools::OptionalToolSet::default())
            .unwrap(),
        OverlayToolPlanDecision::Ready
    );
}

#[test]
fn super_workspace_trust_is_persisted_into_overlay_config() {
    let root = temp_overlay("super-workspace-trust");
    std::fs::create_dir_all(&root).unwrap();
    let args = vec![
        OsString::from("-c"),
        OsString::from("projects={\"/tmp/super-workspace\"={trust_level=\"trusted\"}}"),
        OsString::from("resume"),
        OsString::from("01900000-0000-7000-8000-000000000777"),
    ];

    project_super_workspace_trust(&root, &args).unwrap();

    let rendered = std::fs::read_to_string(root.join("config.toml")).unwrap();
    let config: toml::Value = toml::from_str(&rendered).unwrap();
    assert_eq!(
        config["projects"]["/tmp/super-workspace"]["trust_level"].as_str(),
        Some("trusted")
    );
    assert_eq!(args[2], OsString::from("resume"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn local_super_resume_disables_implicit_daemon_reuse() {
    let mut args = vec![
        OsString::from("resume"),
        OsString::from("01900000-0000-7000-8000-000000000778"),
    ];

    ensure_local_super_uses_owned_server(true, false, &mut args);

    assert_eq!(args.first(), Some(&OsString::from("--no-daemon")));
    assert!(args.iter().any(|arg| arg == "resume"));
}

#[test]
fn local_super_private_companion_does_not_add_no_daemon() {
    let mut args = Vec::new();

    ensure_local_super_uses_owned_server(true, true, &mut args);

    assert!(!args.iter().any(|arg| arg == "--no-daemon"));
}

#[cfg(unix)]
#[test]
fn private_companion_becomes_the_tui_remote_server() {
    let mut child = prodex_runtime_launch::ChildProcessPlan::new(
        OsString::from("codex"),
        PathBuf::from("/tmp/codex-home"),
    );
    let socket = PathBuf::from("/tmp/prodex-super-private.sock");

    connect_child_to_private_companion(&mut child, &socket);

    assert_eq!(child.args.first(), Some(&OsString::from("--remote")));
    assert_eq!(
        child.args.get(1),
        Some(&OsString::from("unix:///tmp/prodex-super-private.sock"))
    );
}

#[test]
fn explicit_remote_super_transport_is_preserved() {
    let mut args = vec![
        OsString::from("--remote"),
        OsString::from("unix:///tmp/user-selected.sock"),
    ];

    ensure_local_super_uses_owned_server(true, false, &mut args);

    assert!(!args.iter().any(|arg| arg == "--no-daemon"));
}

#[test]
fn hook_trust_bypass_stays_cli_only_in_overlay_config() {
    let root = temp_overlay("hook-trust-overlay");
    std::fs::create_dir_all(&root).unwrap();
    let args = vec![
        OsString::from("--dangerously-bypass-hook-trust"),
        OsString::from("-c"),
        OsString::from("disable_paste_burst=true"),
    ];

    configure_overlay_codex_home(&root, &args, false).unwrap();

    let rendered = std::fs::read_to_string(root.join("config.toml")).unwrap();
    let config: toml::Value = toml::from_str(&rendered).unwrap();
    assert!(config.get("bypass_hook_trust").is_none());
    assert_eq!(
        config
            .get("disable_paste_burst")
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert!(
        args.iter()
            .any(|arg| arg == "--dangerously-bypass-hook-trust")
    );

    let _ = std::fs::remove_dir_all(root);
}
