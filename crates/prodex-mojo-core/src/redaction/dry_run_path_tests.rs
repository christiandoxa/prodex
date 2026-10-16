use super::*;

#[test]
fn mojo_dry_run_path_replacements_preserve_order_duplicates_and_unicode() {
    let paths = vec![
        "/home/user".to_owned(),
        "/home/user/private".to_owned(),
        "/home/user/private".to_owned(),
        "".to_owned(),
        "/tmp/東京🧭".to_owned(),
    ];
    assert_eq!(
        redact_dry_run_paths(
            "/home/user/private/file /home/user/other /tmp/東京🧭/state",
            &paths,
        ),
        Ok("<redacted-path>/file <redacted-path>/other <redacted-path>/state".to_owned()),
    );
    assert_eq!(
        redact_dry_run_paths("unchanged", &paths),
        Ok("unchanged".to_owned()),
    );
    assert_eq!(redact_dry_run_paths("", &paths), Ok(String::new()));
    assert_eq!(
        redact_dry_run_paths("/".repeat(128).as_str(), &["/".to_owned()]),
        Ok("<redacted-path>".repeat(128)),
    );
    assert_eq!(
        redact_dry_run_paths(
            "abc def abc",
            &["abc".to_owned(), "def".to_owned(), "abc".to_owned(),]
        ),
        Ok("<redacted-path> <redacted-path> <redacted-path>".to_owned()),
    );
    assert_eq!(
        redact_dry_run_paths("original", &[]),
        Ok("original".to_owned()),
    );
}

#[test]
fn mojo_dry_run_path_abi_rejects_bad_views_and_verifies_exact_capacity() {
    let value = "/private";
    let path = "/private";
    let mut count = 77_i64;
    let mut indices = [99_i64; 16];
    let mut written = -1_i64;
    let mut output = [88_u8; 16];
    let span = DryRunPathView {
        address: path.as_ptr() as u64,
        length: path.len() as u64,
    };
    unsafe {
        assert_eq!(
            prodex_redaction_dry_run_path_order_v1(
                REDACTION_ABI_VERSION + 1,
                &span as *const _ as u64,
                1,
                indices.as_mut_ptr() as u64,
                &mut count as *mut _ as u64,
            ),
            4
        );
        assert_eq!(
            prodex_redaction_dry_run_path_order_v1(
                REDACTION_ABI_VERSION,
                0,
                1,
                indices.as_mut_ptr() as u64,
                &mut count as *mut _ as u64,
            ),
            1
        );
        assert_eq!(
            prodex_redaction_dry_run_path_replace_v1(
                REDACTION_ABI_VERSION,
                value.as_ptr() as u64,
                value.len() as i64,
                path.as_ptr() as u64,
                path.len() as i64,
                output.as_mut_ptr() as u64,
                5,
                &mut written as *mut _ as u64,
            ),
            2
        );
        assert_eq!(written, "<redacted-path>".len() as i64);
        assert_eq!(
            prodex_redaction_dry_run_path_replace_v1(
                REDACTION_ABI_VERSION + 1,
                value.as_ptr() as u64,
                value.len() as i64,
                path.as_ptr() as u64,
                path.len() as i64,
                output.as_mut_ptr() as u64,
                16,
                &mut written as *mut _ as u64,
            ),
            4
        );
    }
    assert_eq!(indices, [99; 16]);
    assert_eq!(count, 77);
    assert_eq!(output, [88; 16]);
}
