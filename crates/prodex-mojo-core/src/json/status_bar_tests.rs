use super::{StatusHeaderInput, status_footer_json, status_header_json};

#[test]
fn status_bar_bridge_preserves_complete_loading_header_refresh_state_and_default_footer() {
    assert_eq!(
        status_header_json(None, false).unwrap(),
        r#"[["title"," PRODEX STATUS "],["accent"," profile loading "],["secondary"," live · waiting for first snapshot "]]"#
    );
    assert_eq!(
        status_header_json(
            Some(StatusHeaderInput {
                profile: "test-profile",
                updated_at: "synthetic-time"
            }),
            true
        )
        .unwrap(),
        r#"[["title"," PRODEX STATUS "],["accent"," profile test-profile "],["secondary"," refreshing · synthetic-time "]]"#
    );
    assert_eq!(
        status_footer_json(None).unwrap(),
        r#"[["hint_bold"," q/esc "],["raw","quit  "],["hint_bold","r "],["raw","refresh  "],["muted","NET shows Prodex socket queues; disk rates come from /proc/<pid>/io"]]"#
    );
}

#[test]
fn status_bar_bridge_preserves_empty_presence_and_escapes_snapshot_and_error_text() {
    assert_eq!(
        status_header_json(
            Some(StatusHeaderInput {
                profile: " \t雪\"\\\n ",
                updated_at: ""
            }),
            false
        )
        .unwrap(),
        r#"[["title"," PRODEX STATUS "],["accent"," profile  \t雪\"\\\n  "],["secondary"," live ·  "]]"#
    );
    assert_eq!(
        status_footer_json(Some("")).unwrap(),
        r#"[["error"," refresh error: "],["muted",""]]"#
    );
    assert_eq!(
        status_footer_json(Some("error\0\n雪")).unwrap(),
        r#"[["error"," refresh error: "],["muted","error\u0000\n雪"]]"#
    );
}
