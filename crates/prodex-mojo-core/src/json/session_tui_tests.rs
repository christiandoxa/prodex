use super::{
    RowFfi, SessionTuiMode, SessionTuiRow, prodex_session_tui_rows_v1, session_tui_rows_json,
};
use crate::json::StringView;

#[test]
fn compiled_session_rows_preserve_complete_inline_scroll_and_empty_outputs() {
    let row = SessionTuiRow {
        id: "session-雪",
        title: None,
        updated_at: None,
        cwd: None,
        profile: None,
        provider: None,
    };
    let rows = [row];
    assert_eq!(
        session_tui_rows_json(&rows, SessionTuiMode::Inline).unwrap(),
        r#"[[[["hint_bold","session-雪"],["raw","  "],["primary","Untitled session"]],[["secondary","updated "],["raw","-"],["secondary"," profile "],["raw","-"],["secondary"," provider "],["raw","-"]],[["secondary","cwd "],["raw","-"]],[]]]"#
    );
    assert_eq!(
        session_tui_rows_json(&rows, SessionTuiMode::Scroll).unwrap(),
        r#"[[[["hint_bold","session-雪"],["raw","  "],["primary","Untitled session"]],[["secondary","updated "],["raw","-"],["secondary","  profile "],["raw","-"],["secondary","  provider "],["raw","-"]],[]]]"#
    );
    assert_eq!(
        session_tui_rows_json(&[], SessionTuiMode::Scroll).unwrap(),
        "[]"
    );
}

#[test]
fn compiled_session_rows_keep_present_empty_unicode_and_native_row_stride() {
    let rows = [
        SessionTuiRow {
            id: "session-雪",
            title: Some(" \n雪\t "),
            updated_at: Some(""),
            cwd: Some("/home/test-user/project"),
            profile: Some(""),
            provider: Some("provider\""),
        },
        SessionTuiRow {
            id: "session-blank",
            title: Some("\u{0085}\u{00a0}\u{3000}"),
            updated_at: None,
            cwd: Some(""),
            profile: None,
            provider: None,
        },
    ];
    assert_eq!(
        session_tui_rows_json(&rows, SessionTuiMode::Scroll).unwrap(),
        r#"[[[["hint_bold","session-雪"],["raw","  "],["primary"," \n雪\t "]],[["secondary","updated "],["raw",""],["secondary","  profile "],["raw",""],["secondary","  provider "],["raw","provider\""]],[["secondary","cwd "],["raw","/home/test-user/project"]],[]],[[["hint_bold","session-blank"],["raw","  "],["primary","Untitled session"]],[["secondary","updated "],["raw","-"],["secondary","  profile "],["raw","-"],["secondary","  provider "],["raw","-"]],[["secondary","cwd "],["raw",""]],[]]]"#
    );
}

#[test]
fn session_row_abi_preserves_buffers_on_invalid_fields_and_short_capacity() {
    let mut rows = [RowFfi {
        presence: 0,
        id: "session".into(),
        fields: [StringView::default(); 5],
    }];
    let mut output = [93_u8; 2048];
    let mut metadata = [77_i64, 88];
    let input = rows.as_ptr() as u64;
    let target = output.as_mut_ptr() as u64;
    let meta = metadata.as_mut_ptr() as u64;
    for (version, mode, source, count, phase, capacity, result, expected) in [
        (2, 0, input, 1, 0, 2048, meta, 4),
        (1, 2, input, 1, 0, 2048, meta, 1),
        (1, 0, input + 1, 1, 0, 2048, meta, 1),
        (1, 0, 0, 1, 0, 2048, meta, 1),
        (1, 0, input, -1, 0, 2048, meta, 1),
        (1, 0, input, 1, 2, 2048, meta, 1),
        (1, 0, input, 1, 1, 2048, meta, 1),
        (1, 0, input, 1, 0, 2048, meta + 1, 1),
        (1, 0, input, 1, 0, 1, meta, 3),
    ] {
        assert_eq!(
            unsafe {
                prodex_session_tui_rows_v1(
                    version, mode, source, count, phase, target, capacity, result,
                )
            },
            expected
        );
        assert_eq!(output, [93; 2048]);
        assert_eq!(metadata, [77, 88]);
    }
    rows[0].presence = 32;
    assert_eq!(
        unsafe { prodex_session_tui_rows_v1(1, 0, rows.as_ptr() as u64, 1, 0, target, 2048, meta) },
        1
    );
    rows[0].presence = 0;
    let bad = [255_u8];
    rows[0].id = StringView {
        address: bad.as_ptr() as u64,
        length: 1,
    };
    assert_eq!(
        unsafe { prodex_session_tui_rows_v1(1, 0, rows.as_ptr() as u64, 1, 0, target, 2048, meta) },
        1
    );
    assert_eq!(output, [93; 2048]);
    assert_eq!(metadata, [77, 88]);
}
