use super::*;

#[test]
fn option_values_and_equals_syntax_keep_the_expose_position() {
    let Commands::Super(args) = parse_cli_command_from(["prodex", "super", "--profile", "expose"])
        .expect("profile value should remain attached to its option")
    else {
        panic!("expose option value must not select the expose command");
    };
    assert_eq!(args.profile.as_deref(), Some("expose"));

    let Commands::SuperExpose(args) =
        parse_cli_command_from(["prodex", "s", "--profile=main", "expose"])
            .expect("equals value should leave the following alias visible")
    else {
        panic!("equals syntax should still find the expose command");
    };
    assert_eq!(args.super_args.profile.as_deref(), Some("main"));
}

#[test]
fn literal_boundary_keeps_expose_as_codex_input() {
    let Commands::Super(args) = parse_cli_command_from(["prodex", "super", "--", "expose"])
        .expect("literal expose should remain a Super argument")
    else {
        panic!("literal boundary must stop alias rewriting");
    };
    assert_eq!(args.codex_args, os_args(&["--", "expose"]));
}

#[test]
fn super_cli_normalization_preserves_provider_aliases_and_literal_tail() {
    for provider in ["gemini", "deepseek"] {
        let command = parse_cli_command_from(["prodex", "s", provider, "exec", "review"])
            .expect("provider shorthand should parse through the production CLI caller");
        let Commands::Super(args) = command else {
            panic!("expected Super command");
        };
        assert_eq!(args.codex_args, os_args(&["exec", "review"]));
    }

    let command = parse_cli_command_from(["prodex", "s", "--", "gemini", "exec"])
        .expect("literal provider text should remain Codex input");
    let Commands::Super(args) = command else {
        panic!("expected Super command");
    };
    assert_eq!(args.codex_args, os_args(&["--", "gemini", "exec"]));
}

#[cfg(unix)]
#[test]
fn opaque_os_arguments_are_preserved_before_and_after_rewrite() {
    use std::os::unix::ffi::OsStringExt;

    let opaque = OsString::from_vec(vec![0xff, b'x']);
    let mut before_alias = os_args(&["prodex", "super"]);
    before_alias.extend([opaque.clone(), OsString::from("expose")]);
    let Commands::Super(args) =
        parse_cli_command_from(before_alias).expect("opaque argument should stop the scan")
    else {
        panic!("opaque argument before expose must preserve the Super command");
    };
    assert_eq!(args.codex_args, [opaque.clone(), OsString::from("expose")]);

    let after_alias = vec![
        OsString::from("prodex"),
        OsString::from("s"),
        OsString::from("expose"),
        OsString::from("full"),
        OsString::from("--"),
        opaque.clone(),
    ];
    let Commands::SuperExpose(args) =
        parse_cli_command_from(after_alias).expect("opaque tail should survive reassembly")
    else {
        panic!("expose alias should rewrite before the opaque tail");
    };
    assert_eq!(args.super_args.codex_args, [opaque]);
}
