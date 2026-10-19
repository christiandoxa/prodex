use super::super::{InfoStringView, prodex_terminal_info_render_v1};
use super::*;

#[test]
fn compiled_preflight_plans_preserve_all_rotation_and_stop_messages() {
    let hint = "Inspect with `prodex quota --profile main` or bypass with `prodex run --skip-quota-check`.".to_string();
    let blocked_message = "Quota preflight blocked profile 'main': exhausted".to_string();
    for (rotate, alternatives, expected) in [
        (
            true,
            vec!["βeta".to_string(), "other".to_string()],
            RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
                blocked_message: blocked_message.clone(),
                next_profile: "βeta".to_string(),
                rotate_message: "Auto-rotating to profile 'βeta'.".to_string(),
            },
        ),
        (
            true,
            vec![],
            RuntimeLaunchBlockedSelectedProfilePlan::Stop {
                blocked_message: blocked_message.clone(),
                messages: vec![
                    "No other ready profile was found.".to_string(),
                    hint.clone(),
                ],
                error_message:
                    "quota preflight blocked profile 'main' and no other ready profile was found"
                        .to_string(),
            },
        ),
        (
            false,
            vec![],
            RuntimeLaunchBlockedSelectedProfilePlan::Stop {
                blocked_message: blocked_message.clone(),
                messages: vec![hint.clone()],
                error_message: "quota preflight blocked profile 'main' with auto-rotate disabled"
                    .to_string(),
            },
        ),
        (
            false,
            vec!["βeta".to_string(), "".to_string()],
            RuntimeLaunchBlockedSelectedProfilePlan::Stop {
                blocked_message: blocked_message.clone(),
                messages: vec![
                    "Other profiles that look ready: βeta, ".to_string(),
                    "Rerun without `--no-auto-rotate` to allow fallback.".to_string(),
                    hint.clone(),
                ],
                error_message: "quota preflight blocked profile 'main' with auto-rotate disabled"
                    .to_string(),
            },
        ),
    ] {
        assert_eq!(
            blocked_selected_runtime_profile_plan("main", "exhausted", rotate, &alternatives),
            Ok(expected)
        );
    }
    assert_eq!(
        no_ready_runtime_profiles_plan(
            "main",
            InfoRuntimeLaunchSelectedProfileStatus::Blocked {
                blocked_summary: "exhausted"
            }
        ),
        Ok(RuntimeLaunchNoReadyProfilesPlan::Blocked {
            blocked_message,
            no_ready_message: "No ready profile was found.".to_string(),
            inspect_hint: hint,
            error_message: "quota preflight blocked profile 'main' and no ready profile was found"
                .to_string(),
        })
    );
    assert_eq!(
        no_ready_runtime_profiles_plan(
            "main",
            InfoRuntimeLaunchSelectedProfileStatus::ProbeFailed {
                error: "timeout: detail"
            }
        ),
        Ok(RuntimeLaunchNoReadyProfilesPlan::ProbeFailed {
            warning_message: "Warning: quota preflight failed for 'main': timeout: detail"
                .to_string(),
            continue_message: "Continuing without quota gate.".to_string(),
        })
    );
}

#[test]
fn compiled_preflight_frames_preserve_long_unicode_nul_and_delimiters() {
    let profile = "β🐴\0:;".repeat(4096);
    let detail = "quota\0:;\n".repeat(256);
    let plan = blocked_selected_runtime_profile_plan(
        &profile,
        &detail,
        false,
        std::slice::from_ref(&profile),
    )
    .unwrap();
    assert_eq!(
        plan,
        RuntimeLaunchBlockedSelectedProfilePlan::Stop {
            blocked_message: format!("Quota preflight blocked profile '{profile}': {detail}"),
            messages: vec![
                format!("Other profiles that look ready: {profile}"),
                "Rerun without `--no-auto-rotate` to allow fallback.".to_string(),
                format!(
                    "Inspect with `prodex quota --profile {profile}` or bypass with `prodex run --skip-quota-check`."
                ),
            ],
            error_message: format!(
                "quota preflight blocked profile '{profile}' with auto-rotate disabled"
            ),
        }
    );
}

#[test]
fn preflight_frame_decoder_rejects_bad_width_overflow_truncation_and_utf8() {
    for output in [
        b"".as_slice(),
        b"0x:",
        b"000000000000000000002:x",
        b"018446744073709551616:x",
        b"000000000000000000001:\xff",
    ] {
        assert_eq!(plan_fields(output), Err(MojoError::InvalidOutput));
    }
    let too_many = format!("0{}", "00000000000000000000:".repeat(6));
    assert_eq!(
        plan_fields(too_many.as_bytes()),
        Err(MojoError::InvalidOutput)
    );
    assert_eq!(
        no_ready_runtime_profiles_plan("main", InfoRuntimeLaunchSelectedProfileStatus::Ready),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_preflight_abi_rejects_invalid_shapes_before_writes() {
    let texts = ["main", "", "", "quota", "alt"];
    let views = texts.map(|value| InfoStringView {
        ptr: value.as_ptr() as u64,
        len: value.len() as u64,
    });
    let mut output = [71_u8; 512];
    let mut written = -1_i64;
    for (operation, count, flags, capacity, expected) in [
        (LAUNCH_NO_READY, 4, 2, 512, 1),
        (LAUNCH_NO_READY, 5, 0, 512, 1),
        (LAUNCH_BLOCKED, 3, 0, 512, 1),
        (LAUNCH_BLOCKED, 5, 2, 512, 1),
        (LAUNCH_BLOCKED, 5, 0, 0, 3),
    ] {
        assert_eq!(
            unsafe {
                prodex_terminal_info_render_v1(
                    1,
                    operation,
                    0,
                    0,
                    0,
                    0,
                    views.as_ptr() as u64,
                    count,
                    flags,
                    output.as_mut_ptr() as u64,
                    capacity,
                    (&mut written as *mut i64) as u64,
                )
            },
            expected
        );
        assert_eq!(output, [71; 512]);
        assert_eq!(written, if capacity == 0 { 0 } else { -1 });
    }
}
