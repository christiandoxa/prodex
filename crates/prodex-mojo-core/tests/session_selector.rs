#[repr(C)]
#[derive(Clone, Copy)]
struct View {
    address: u64,
    length: u64,
}

unsafe extern "C" {
    fn prodex_session_report_query_matches_v1(
        abi_version: i64,
        query_address: u64,
        query_length: i64,
        fields_address: u64,
        fields_count: i64,
        output_address: u64,
    ) -> i64;
}

fn view(value: &str) -> View {
    View {
        address: value.as_ptr() as u64,
        length: value.len() as u64,
    }
}

#[test]
fn direct_query_abi_matches_normalized_unicode_and_rejects_wrong_field_count() {
    let query = "i\u{307}stanbul";
    let values = [
        "01900000-0000-7000-8000-000000000301",
        "straße i\u{307}stanbul",
        "/home/test-user/workspace",
        "main",
        "openai",
        "/home/test-user/.codex/sessions/unicode.jsonl",
    ];
    let fields = values.map(view);
    let mut output = [-1_i64; 1];
    let status = unsafe {
        prodex_session_report_query_matches_v1(
            1,
            query.as_ptr() as u64,
            query.len() as i64,
            fields.as_ptr() as u64,
            fields.len() as i64,
            output.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(output, [1]);

    let mut invalid_output = [-1_i64; 1];
    let invalid_status = unsafe {
        prodex_session_report_query_matches_v1(
            1,
            query.as_ptr() as u64,
            query.len() as i64,
            fields.as_ptr() as u64,
            (fields.len() - 1) as i64,
            invalid_output.as_mut_ptr() as u64,
        )
    };
    assert_eq!(invalid_status, 1);
    assert_eq!(invalid_output, [-1]);

    let borrowed = values.map(Some);
    assert!(prodex_mojo_core::json::session_report_query_matches(query, &borrowed).unwrap());
}
