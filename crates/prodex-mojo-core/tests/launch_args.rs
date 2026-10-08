#![cfg(feature = "mojo-runtime")]
#![allow(unsafe_code)]

use prodex_mojo_core::launch::{
    LaunchArgument, LaunchArgumentOperation, SuperValidationInput, SuperValidationScope,
    SuperValidationViolation, default_cli_invocation_to_run, find_super_expose_alias_index,
    inspect_launch_arguments, plan_launch_arguments, plan_super_validation,
};

#[test]
fn default_cli_invocation_policy_uses_real_mojo_classification() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    let prodex_commands = [
        "-h",
        "--help",
        "-V",
        "--version",
        "profile",
        "use",
        "current",
        "info",
        "status",
        "log",
        "session",
        "doctor",
        "login",
        "logout",
        "update",
        "quota",
        "redeem",
        "ping",
        "run",
        "super",
        "s",
        "gateway",
        "gui",
        "dashboard",
        "claude",
        "help",
        "__super-expose",
        "__runtime-broker",
        "__mcp-jsonl-bridge",
        "__sub-agent-exec",
    ];
    for command in prodex_commands {
        assert!(
            !default_cli_invocation_to_run(&[Some("prodex"), Some(command)]).unwrap(),
            "{command} must remain a top-level command"
        );
    }

    assert!(default_cli_invocation_to_run(&[]).unwrap());
    assert!(default_cli_invocation_to_run(&[Some("prodex")]).unwrap());
    assert!(default_cli_invocation_to_run(&[Some("prodex"), None]).unwrap());
    assert!(default_cli_invocation_to_run(&[Some("prodex"), Some("remote-control")]).unwrap());
    assert!(default_cli_invocation_to_run(&[Some("prodex"), Some("mcp-server")]).unwrap());
    assert!(!default_cli_invocation_to_run(&[None, Some("--help")]).unwrap());
}

#[test]
fn super_expose_alias_scan_matches_versioned_mojo_abi_values() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    let cases = [
        (vec![Some("prodex"), Some("super"), Some("expose")], Some(2)),
        (
            vec![
                Some("prodex"),
                Some("s"),
                Some("--profile"),
                Some("main"),
                Some("expose"),
            ],
            Some(4),
        ),
        (
            vec![
                Some("prodex"),
                Some("super"),
                Some("--profile"),
                Some("expose"),
            ],
            None,
        ),
        (
            vec![
                Some("prodex"),
                Some("super"),
                Some("--profile=main"),
                Some("expose"),
            ],
            Some(3),
        ),
        (
            vec![Some("prodex"), Some("super"), Some("--"), Some("expose")],
            None,
        ),
        (
            vec![Some("prodex"), Some("super"), Some("exec"), Some("expose")],
            None,
        ),
        (
            vec![Some("prodex"), Some("super"), None, Some("expose")],
            None,
        ),
        (
            vec![
                Some("prodex"),
                Some("super"),
                Some("--profile"),
                None,
                Some("expose"),
            ],
            Some(4),
        ),
        (
            vec![Some("prodex"), Some("super"), Some("--cli"), Some("expose")],
            Some(3),
        ),
    ];
    for (arguments, expected) in cases {
        assert_eq!(find_super_expose_alias_index(&arguments).unwrap(), expected);
    }

    for option in [
        "-p",
        "--profile",
        "--base-url",
        "--sub-agent-provider",
        "--sub-agent-model",
        "--sub-agent-model-reasoning-effort",
        "--sub-agent-url",
        "--sub-agent-max-concurrency",
        "--tool",
        "--require-tool",
        "--url",
        "--provider",
        "--api-key",
        "--model",
        "--local-model",
        "--context-window",
        "--local-context-window",
        "--auto-compact-token-limit",
        "--local-auto-compact-token-limit",
        "-c",
    ] {
        assert_eq!(
            find_super_expose_alias_index(&[
                Some("prodex"),
                Some("s"),
                Some(option),
                Some("expose"),
            ])
            .unwrap(),
            None,
            "{option} must consume its separate value"
        );
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct View {
    address: u64,
    length: u64,
    valid_utf8: i64,
}

unsafe extern "C" {
    fn prodex_mojo_launch_args_v1(
        version: i64,
        operation: i64,
        full: i64,
        args: u64,
        count: i64,
        out: u64,
        capacity: i64,
        scratch: u64,
        meta: u64,
    ) -> i64;
}

#[test]
fn launch_boundary_accepts_empty_and_opaque_records() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    let plan = plan_launch_arguments(&[], LaunchArgumentOperation::RetargetExec, false).unwrap();
    assert_eq!(
        plan.arguments,
        vec![
            LaunchArgument::Exec,
            LaunchArgument::Resume,
            LaunchArgument::Session
        ]
    );
    let opaque = [None, Some("--profile-v2=x")];
    let plan =
        plan_launch_arguments(&opaque, LaunchArgumentOperation::NormalizeRun, false).unwrap();
    assert_eq!(
        plan.arguments,
        vec![LaunchArgument::Original(0), LaunchArgument::Original(1)]
    );
    assert_eq!(
        inspect_launch_arguments(&opaque).unwrap().first_positional,
        Some(0)
    );
}

#[test]
fn launch_boundary_rejects_bad_abi_tags_utf8_and_capacities_without_writes() {
    let bytes = [0xff_u8];
    let input = [View {
        address: bytes.as_ptr() as u64,
        length: 1,
        valid_utf8: 1,
    }];
    let mut out = [0x5a_i64; 12];
    let mut scratch = [0x5a_i64; 12];
    let mut meta = [0x5a_i64; 11];
    for (version, op, full, address, count, capacity, expected) in [
        (2, 1, 0, 0, 0, 4, 4),
        (1, 99, 0, 0, 0, 4, 1),
        (1, 4, 0, 0, 0, 4, 1),
        (1, 5, 0, 0, 0, 4, 1),
        (1, 1, 2, 0, 0, 4, 1),
        (1, 1, 0, 0, -1, 4, 1),
        (1, 1, 0, 0, 1, 4, 1),
        (1, 1, 0, 0, 0, 2, 3),
        (1, 1, 0, input.as_ptr() as u64, 1, 4, 2),
    ] {
        let result = unsafe {
            prodex_mojo_launch_args_v1(
                version,
                op,
                full,
                address,
                count,
                out.as_mut_ptr() as u64,
                capacity,
                scratch.as_mut_ptr() as u64,
                meta.as_mut_ptr() as u64,
            )
        };
        assert_eq!(result, expected);
        assert_eq!(out, [0x5a; 12]);
        assert_eq!(scratch, [0x5a; 12]);
        assert_eq!(meta, [0x5a; 11]);
    }
    let mut invalid = View {
        address: 1,
        length: 0,
        valid_utf8: 0,
    };
    for tag in [0, -1, 2] {
        invalid.valid_utf8 = tag;
        let status = unsafe {
            prodex_mojo_launch_args_v1(
                1,
                0,
                0,
                &invalid as *const View as u64,
                1,
                0,
                0,
                0,
                meta.as_mut_ptr() as u64,
            )
        };
        assert_eq!(status, 1);
    }
}

#[test]
fn super_validation_abi_accepts_empty_and_max_flags_and_rejects_bad_limits() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    let mut metadata = [0_i64; 11];
    for operation in [14, 15] {
        let status = unsafe {
            prodex_mojo_launch_args_v1(
                1,
                operation,
                2_097_151,
                0,
                0,
                0,
                0,
                0,
                metadata.as_mut_ptr() as u64,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(metadata[0], 1);
    }

    metadata.fill(0x5a);
    for (version, operation, flags, count, expected) in [
        (1, 14, 2_097_152, 0, 1),
        (1, 16, 0, 0, 1),
        (1, 14, 0, i64::MAX, 1),
        (2, 14, 0, 0, 4),
    ] {
        let status = unsafe {
            prodex_mojo_launch_args_v1(
                version,
                operation,
                flags,
                0,
                count,
                0,
                0,
                0,
                metadata.as_mut_ptr() as u64,
            )
        };
        assert_eq!(status, expected);
        assert_eq!(metadata, [0x5a; 11]);
    }
}

#[test]
fn launch_model_whitespace_matches_rust_not_python_classification() {
    for text in ["\u{001c}", "\u{001d}", "\u{001e}", "\u{001f}"] {
        assert_eq!(
            inspect_launch_arguments(&[Some("-m"), Some(text)])
                .unwrap()
                .model,
            Some(text)
        );
    }
    for text in ["", "\u{0085}", "\u{00a0}", "\u{2007}", "\u{3000}"] {
        assert_eq!(
            inspect_launch_arguments(&[Some("-m"), Some(text)])
                .unwrap()
                .model,
            None
        );
    }
}

#[test]
fn super_validation_plan_preserves_order_and_sub_agent_scope() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    let validate = |input, arguments: &[Option<&str>], scope| {
        plan_super_validation(input, arguments, scope).unwrap()
    };
    assert_eq!(
        validate(
            SuperValidationInput::default(),
            &[],
            SuperValidationScope::Full
        ),
        None
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                sub_agent: true,
                no_sub_agent: true,
                auto_rotate: true,
                no_auto_rotate: true,
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::SubAgentConflict)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                sub_agent_model: true,
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::SubAgentDetailsRequireEnable)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                sub_agent: true,
                sub_agent_model: true,
                provider_url_violation: Some(
                    prodex_mojo_core::sub_agent_policy::ProviderUrlViolation::LocalRequiresUrl
                ),
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::SubAgentModelEmpty)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                provider_url_violation: Some(
                    prodex_mojo_core::sub_agent_policy::ProviderUrlViolation::LocalRequiresUrl
                ),
                ..Default::default()
            },
            &[],
            SuperValidationScope::SubAgentOnly
        ),
        Some(SuperValidationViolation::LocalSubAgentRequiresUrl)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                provider_url_violation: Some(
                    prodex_mojo_core::sub_agent_policy::ProviderUrlViolation::NonLocalRejectsUrl
                ),
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::SubAgentUrlRequiresLocal)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                auto_rotate: true,
                no_auto_rotate: true,
                presidio: true,
                no_presidio: true,
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::AutoRotateConflict)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                presidio: true,
                no_presidio: true,
                required_presidio: true,
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::PresidioConflict)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                no_presidio: true,
                required_presidio: true,
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::NoPresidioRequiresPresidioTool)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                provider: true,
                url: true,
                base_url: true,
                api_key: true,
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::ProviderUrlConflict)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                base_url: true,
                url: true,
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::BaseUrlUrlConflict)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                api_key: true,
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::ApiKeyRequiresProvider)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                local_context_window: true,
                ..Default::default()
            },
            &[],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::ContextWindowRequiresProviderOrUrl)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                sub_agent: true,
                ..Default::default()
            },
            &[Some("gui")],
            SuperValidationScope::Full
        ),
        Some(SuperValidationViolation::SubAgentUnsupportedWithDesktop)
    );
    assert_eq!(
        validate(
            SuperValidationInput {
                api_key: true,
                ..Default::default()
            },
            &[Some("gui")],
            SuperValidationScope::SubAgentOnly
        ),
        None
    );
}

#[test]
fn launch_boundary_is_reentrant() {
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..200 {
                    let args = [
                        Some("--full-access"),
                        Some("exec"),
                        Some("--"),
                        Some("review"),
                    ];
                    let plan =
                        plan_launch_arguments(&args, LaunchArgumentOperation::Prepare, false)
                            .unwrap();
                    assert_eq!(
                        plan.arguments,
                        vec![
                            LaunchArgument::FullAccess,
                            LaunchArgument::Original(1),
                            LaunchArgument::Original(2),
                            LaunchArgument::Original(3)
                        ]
                    );
                    assert!(!plan.flag);
                }
            });
        }
    });
}
