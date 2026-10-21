use super::*;

#[test]
fn gemini_rtk_mojo_preserves_raw_word_policy_and_wrapping() {
    for (input, expected) in [
        ("git status", Some("rtk git status")),
        ("git branch", None),
        ("echo cargo test", None),
        ("cd /repo && cargo test", Some("cd /repo && rtk cargo test")),
        (
            "X=é　/usr/bin/cargo (test)",
            Some("X=é　rtk /usr/bin/cargo (test)"),
        ),
        (
            "rtk git status || npm test",
            Some("rtk git status || rtk npm test"),
        ),
        ("X=1 rtk cargo test", None),
        ("claw-compactor compact", Some("rtk claw-compactor compact")),
        (
            "claw-compactor summarize",
            Some("rtk claw-compactor summarize"),
        ),
        (
            "prodex-claw-compactor-auto",
            Some("rtk prodex-claw-compactor-auto"),
        ),
        ("'cargo' test", None),
        ("", None),
    ] {
        assert_eq!(
            gemini_rtk_shell_command(input).unwrap().as_deref(),
            expected,
            "{input:?}"
        );
    }
    let long = format!("/{}cargo test", "x/".repeat(5000));
    assert_eq!(
        gemini_rtk_shell_command(&long).unwrap(),
        Some(format!("rtk {long}"))
    );
}

#[test]
fn gemini_rtk_mojo_validates_abi_and_capacity_before_emitting_output() {
    let input = b"cargo test";
    let mut output = [77_u8; 16];
    let mut written = 99_i64;
    let mut call = |abi, capacity| unsafe {
        prodex_gemini_rtk_shell_command_v1(
            abi,
            input.as_ptr() as u64,
            input.len() as i64,
            output.as_mut_ptr() as u64,
            capacity,
            (&mut written as *mut i64) as u64,
        )
    };
    assert_eq!(call(2, 16), 4);
    assert_eq!(call(1, 13), 3);
    assert_eq!(output, [77; 16]);
    assert_eq!(written, 99);
}
