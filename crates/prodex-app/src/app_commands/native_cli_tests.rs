use super::resolve_super_launch_decisions_with_prompts;
use prodex_cli::{SuperArgs, SuperExternalProvider};
use prodex_provider_core::ProviderId;

fn super_args(values: &[&str]) -> SuperArgs {
    let mut argv = vec!["prodex", "s"];
    argv.extend(values.iter().copied());
    let crate::Commands::Super(mut args) =
        crate::parse_cli_command_from(argv).expect("Super command should parse")
    else {
        panic!("expected Super command");
    };
    args.extract_super_overrides_from_codex_args()
        .expect("Super overrides should parse");
    *args
}

#[test]
fn native_kiro_resolution_preserves_explicit_provider_without_prompt() {
    let mut args = super_args(&["--provider", "kiro", "--no-sub-agent"]);
    let (_, main_agent, _) = resolve_super_launch_decisions_with_prompts(
        &mut args,
        false,
        || panic!("native Kiro must not prompt for Presidio"),
        |_| Ok(None),
        |_, _| panic!("explicit native Kiro must skip the picker"),
        |_| panic!("explicit --no-sub-agent must skip the prompt"),
    )
    .unwrap();

    assert_eq!(main_agent.provider, ProviderId::Kiro);
    assert_eq!(args.provider, Some(SuperExternalProvider::Kiro));
}
