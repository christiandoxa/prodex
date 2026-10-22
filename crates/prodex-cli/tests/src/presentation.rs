use crate::{Cli, parse_cli_command_from};
use clap::CommandFactory;

#[test]
fn cli_after_help_preserves_all_original_bytes() {
    let root = Cli::command();
    for (name, expected_length, expected_hash) in [
        (None, 426, 0x82460174480663e0_u64),
        (Some("profile"), 827, 0x542f61f5b923e4ff_u64),
        (Some("login"), 724, 0xd5f3c28c01183558_u64),
        (Some("quota"), 1239, 0x681bf5372c6943af_u64),
        (Some("redeem"), 511, 0xb3a64c323850321a_u64),
        (Some("run"), 980, 0x248df9759a8e94d3_u64),
        (Some("super"), 4483, 0x9901b75cb564c7d0_u64),
        (Some("doctor"), 230, 0xfe4dbd8f55073010_u64),
        (Some("session"), 355, 0xc11a6d0aafde817a_u64),
    ] {
        let command = name.map_or(&root, |name| {
            root.find_subcommand(name).expect("help command")
        });
        let text = command
            .get_after_help()
            .expect("after-help text")
            .to_string();
        let hash = text.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        assert_eq!(
            (text.len(), hash),
            (expected_length, expected_hash),
            "{name:?}"
        );
    }
}

#[test]
fn cli_process_metadata_preserves_every_canonical_command() {
    let cases: &[(&[&str], &str, bool)] = &[
        (&["prodex", "profile", "list"], "profile", false),
        (&["prodex", "use", "--profile", "synthetic"], "use", false),
        (&["prodex", "current"], "current", false),
        (&["prodex", "info"], "info", false),
        (&["prodex", "status"], "status", false),
        (&["prodex", "log"], "log", false),
        (&["prodex", "session", "list"], "session", false),
        (&["prodex", "doctor"], "doctor", false),
        (&["prodex", "login"], "login", false),
        (&["prodex", "logout"], "logout", false),
        (&["prodex", "update"], "update", false),
        (&["prodex", "quota", "--once"], "quota", false),
        (&["prodex", "redeem", "synthetic"], "redeem", false),
        (&["prodex", "ping", "openai"], "ping", false),
        (&["prodex", "run"], "run", true),
        (&["prodex", "super"], "super", true),
        (&["prodex", "gateway"], "gateway", true),
        (&["prodex", "__super-expose"], "super-expose", true),
        (&["prodex", "__runtime-broker"], "__runtime-broker", true),
        (
            &["prodex", "__mcp-jsonl-bridge", "synthetic-command"],
            "__mcp-jsonl-bridge",
            false,
        ),
        (
            &[
                "prodex",
                "__sub-agent-exec",
                "--config",
                "config.json",
                "--task-file",
                "task.txt",
            ],
            "__sub-agent-exec",
            false,
        ),
    ];
    for &(args, label, runtime) in cases {
        let command = parse_cli_command_from(args.iter().copied()).expect("canonical command");
        assert_eq!(
            (command.process_label(), command.launches_runtime()),
            (label, runtime),
            "{args:?}"
        );
    }
}
