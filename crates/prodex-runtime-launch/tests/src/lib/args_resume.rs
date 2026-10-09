use super::*;

#[test]
fn codex_resume_last_prompt_is_not_treated_as_session_id() {
    let args = vec![
        OsString::from("resume"),
        OsString::from("--last"),
        OsString::from("continue from current context"),
    ];

    assert_eq!(codex_resume_session_id(&args), None);
    assert_eq!(normalize_run_codex_args(&args), args);
}

#[test]
fn codex_fork_last_prompt_survives_launch_normalization() {
    let args = vec![
        OsString::from("fork"),
        OsString::from("--last"),
        OsString::from("continue from current context"),
    ];

    let (normalized, include_code_review) = prepare_codex_launch_args(&args, false);

    assert_eq!(normalized, args);
    assert!(!include_code_review);
}

#[test]
fn prepare_codex_launch_args_normalizes_resume_after_provider_config_overrides() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("--config=model=\"gemini-2.5-pro\""),
            OsString::from("-cmodel_context_window=1048576"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ],
        false,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("--config=model=\"gemini-2.5-pro\""),
            OsString::from("-cmodel_context_window=1048576"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ]
    );
    assert!(!include_code_review);
}

#[test]
fn bare_resume_normalization_drops_thread_source_for_the_existing_thread() {
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("--thread-source"),
            OsString::from("should_not_override"),
            OsString::from(session_id),
        ],
        false,
    );

    assert_eq!(args, [OsString::from("resume"), OsString::from(session_id)]);
    assert!(!include_code_review);
}

#[test]
fn codex_resume_session_id_extracts_normalized_resume_target() {
    let (args, _) = prepare_codex_launch_args(
        &[
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-gemini\""),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ],
        false,
    );

    assert_eq!(
        codex_resume_session_id(&args),
        Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
    );
}

#[test]
fn codex_resume_session_id_extracts_explicit_exec_resume_target() {
    let args = [
        OsString::from("exec"),
        OsString::from("resume"),
        OsString::from("-c"),
        OsString::from("model_provider=\"prodex-gemini\""),
        OsString::from("--config=model=\"auto\""),
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("continue"),
    ];

    assert_eq!(
        codex_resume_session_id(&args),
        Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
    );
}

#[test]
fn codex_resume_detection_skips_thread_source_option_values() {
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";
    let args = [
        OsString::from("exec"),
        OsString::from("--thread-source"),
        OsString::from("automated_review"),
        OsString::from("resume"),
        OsString::from(session_id),
        OsString::from("continue"),
    ];

    assert!(codex_resume_requested(&args));
    assert_eq!(codex_resume_session_id(&args), Some(session_id));
}

#[test]
fn codex_resume_session_id_extracts_target_before_prompt() {
    let args = [
        OsString::from("resume"),
        OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        OsString::from("/compact focus on auth"),
    ];

    assert_eq!(
        codex_resume_session_id(&args),
        Some("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9")
    );
}

#[test]
fn retarget_codex_tui_resume_args_preserves_global_options_and_replaces_prompt() {
    let args = retarget_codex_tui_resume_args(
        &[
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("initial prompt"),
        ],
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
    );

    assert_eq!(
        args,
        [
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ]
    );
}

#[test]
fn retarget_codex_tui_resume_args_omits_thread_source() {
    let args = retarget_codex_tui_resume_args(
        &[
            OsString::from("--thread-source=automated_review"),
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("exec"),
            OsString::from("initial prompt"),
        ],
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
    );

    assert_eq!(
        args,
        [
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ]
    );
}

#[test]
fn retarget_codex_exec_resume_args_keeps_headless_exec_mode() {
    let args = retarget_codex_exec_resume_args(
        &[
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("exec"),
            OsString::from("original prompt"),
        ],
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
    );

    assert_eq!(
        args,
        [
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ]
    );
}

#[test]
fn retarget_codex_exec_resume_args_preserves_exec_options_not_prompt() {
    let args = retarget_codex_exec_resume_args(
        &[
            OsString::from("exec"),
            OsString::from("--json"),
            OsString::from("--output-last-message"),
            OsString::from("/tmp/last-message.txt"),
            OsString::from("-c"),
            OsString::from("model_reasoning_effort=\"max\""),
            OsString::from("original prompt"),
        ],
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
    );

    assert_eq!(
        args,
        [
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("--json"),
            OsString::from("--output-last-message"),
            OsString::from("/tmp/last-message.txt"),
            OsString::from("-c"),
            OsString::from("model_reasoning_effort=\"max\""),
        ]
    );
}

#[test]
fn retarget_codex_exec_resume_args_replaces_last_with_exact_session() {
    let session_id = OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9");
    for original in [
        vec![
            OsString::from("exec"),
            OsString::from("--last"),
            OsString::from("prompt"),
        ],
        vec![
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from("--last"),
            OsString::from("prompt"),
        ],
    ] {
        assert_eq!(
            retarget_codex_exec_resume_args(&original, session_id.to_str().unwrap()),
            [
                OsString::from("exec"),
                OsString::from("resume"),
                session_id.clone()
            ]
        );
    }
}

#[test]
fn retarget_codex_exec_resume_args_drops_prompt_before_separator() {
    let args = retarget_codex_exec_resume_args(
        &[
            OsString::from("exec"),
            OsString::from("original prompt"),
            OsString::from("--"),
            OsString::from("literal prompt argument"),
        ],
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
    );

    assert_eq!(
        args,
        [
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ]
    );
}

#[test]
fn retarget_codex_exec_resume_args_drops_stdin_prompt_and_thread_source() {
    let args = retarget_codex_exec_resume_args(
        &[
            OsString::from("exec"),
            OsString::from("--thread-source"),
            OsString::from("automated_review"),
            OsString::from("-"),
        ],
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
    );

    assert_eq!(
        args,
        [
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
        ]
    );
}

#[test]
fn retarget_codex_tui_resume_args_preserves_options_after_prompt() {
    let args = retarget_codex_tui_resume_args(
        &[
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("original prompt"),
            OsString::from("--no-alt-screen"),
            OsString::from("--cd"),
            OsString::from("/home/test-user/workspace"),
        ],
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
    );

    assert_eq!(
        args,
        [
            OsString::from("--model"),
            OsString::from("gpt-5.6"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("--no-alt-screen"),
            OsString::from("--cd"),
            OsString::from("/home/test-user/workspace"),
        ]
    );
}

#[test]
fn codex_resume_session_id_ignores_resume_without_target() {
    assert_eq!(
        codex_resume_session_id(&[OsString::from("resume"), OsString::from("--last")]),
        None
    );
    assert_eq!(
        codex_resume_session_id(&[
            OsString::from("resume"),
            OsString::from("--last"),
            OsString::from("/compact focus on auth"),
        ]),
        None
    );
    assert_eq!(
        codex_resume_session_id(&[
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from("--last"),
            OsString::from("continue from latest"),
        ]),
        None
    );
}

#[test]
fn codex_resume_requested_includes_last_without_classifying_fork() {
    assert!(codex_resume_requested(&[
        OsString::from("resume"),
        OsString::from("--last"),
    ]));
    assert!(codex_resume_requested(&[
        OsString::from("exec"),
        OsString::from("resume"),
        OsString::from("--last"),
    ]));
    assert!(!codex_resume_requested(&[
        OsString::from("exec"),
        OsString::from("fork"),
        OsString::from("--last"),
    ]));
}
