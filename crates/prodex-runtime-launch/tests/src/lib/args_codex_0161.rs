use super::*;
use std::ffi::OsString;

const SESSION: &str = "00000000-0000-4000-8000-000000000001";
const NEXT_SESSION: &str = "00000000-0000-4000-8000-000000000002";

fn arguments(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn codex_0161_resume_identity_skips_the_program_value_in_each_global_position() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    for program in ["standard", "daybreak_blue", "daybreak_red"] {
        for input in [
            vec![
                "exec",
                "--cyber-access-program",
                program,
                "resume",
                SESSION,
                "continue",
            ],
            vec![
                "exec",
                "resume",
                "--cyber-access-program",
                program,
                SESSION,
                "continue",
            ],
            vec![
                "exec",
                "resume",
                SESSION,
                "--cyber-access-program",
                program,
                "continue",
            ],
        ] {
            let args = arguments(&input);
            assert!(codex_resume_requested(&args), "{input:?}");
            assert_eq!(codex_resume_session_id(&args), Some(SESSION), "{input:?}");
            assert_eq!(normalize_run_codex_args(&args), args);
        }
    }
}

#[test]
fn codex_0161_recovery_preserves_program_values_and_explicit_daybreak_settings() {
    for program in ["standard", "daybreak_blue", "daybreak_red"] {
        for input in [
            vec![
                "exec",
                "--cyber-access-program",
                program,
                "-c",
                "daybreak=false",
                "original prompt",
            ],
            vec![
                "exec",
                "resume",
                SESSION,
                "--cyber-access-program",
                program,
                "-c",
                "daybreak=false",
                "original prompt",
            ],
        ] {
            let recovered = retarget_codex_exec_resume_args(&arguments(&input), NEXT_SESSION);
            assert_eq!(
                recovered,
                arguments(&[
                    "exec",
                    "resume",
                    NEXT_SESSION,
                    "--cyber-access-program",
                    program,
                    "-c",
                    "daybreak=false",
                ]),
                "{input:?}"
            );
            assert_eq!(codex_resume_session_id(&recovered), Some(NEXT_SESSION));
        }
    }
}

#[test]
fn codex_0161_proxy_config_does_not_split_program_pairs_or_change_resume_identity() {
    let address = "127.0.0.1:12345".parse().unwrap();
    let plain = arguments(&["exec", "resume", SESSION, "continue"]);
    let mut expected = runtime_proxy_codex_args(address, &plain);
    expected.splice(1..1, arguments(&["--cyber-access-program", "standard"]));
    let input = arguments(&[
        "exec",
        "--cyber-access-program",
        "standard",
        "resume",
        SESSION,
        "continue",
    ]);
    let actual = runtime_proxy_codex_args(address, &input);
    assert_eq!(actual, expected);
    assert!(codex_resume_requested(&actual));
    assert_eq!(codex_resume_session_id(&actual), Some(SESSION));
}

#[test]
fn codex_0161_inline_program_and_literal_prompt_survive_launch_planning() {
    let inline = "--cyber-access-program=daybreak_blue";
    let input = arguments(&[
        "exec",
        inline,
        "resume",
        SESSION,
        "--",
        "literal --cyber-access-program text",
    ]);
    assert_eq!(codex_resume_session_id(&input), Some(SESSION));
    assert_eq!(
        prepare_codex_launch_args(&input, false),
        (input.clone(), false)
    );
    assert_eq!(
        retarget_codex_exec_resume_args(&input, NEXT_SESSION),
        arguments(&["exec", "resume", NEXT_SESSION, inline]),
    );
}

#[test]
fn codex_0161_missing_or_invalid_program_is_not_a_session_or_rewritten_default() {
    for tail in [
        vec!["--cyber-access-program"],
        vec!["--cyber-access-program", SESSION],
    ] {
        let mut input = arguments(&["exec", "resume"]);
        input.extend(arguments(&tail));
        assert!(codex_resume_requested(&input));
        assert_eq!(codex_resume_session_id(&input), None, "{input:?}");
        assert_eq!(
            prepare_codex_launch_args(&input, false),
            (input.clone(), false)
        );
    }
    let input = arguments(&["exec", "resume", "--", "--cyber-access-program", SESSION]);
    assert_eq!(codex_resume_session_id(&input), None);
    assert_eq!(
        prepare_codex_launch_args(&input, false),
        (input.clone(), false)
    );
}
