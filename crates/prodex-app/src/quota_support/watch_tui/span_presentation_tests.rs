use super::{quota_human_tui_spans, quota_human_tui_text, quota_watch_status_style};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

fn quota_tui_corpus() -> Value {
    let mut cases = [
        "",
        "-",
        " \t\n\r",
        "-\u{2003}-\u{00a0}",
        "\u{001c}",
        "\u{200b}",
        "== title",
        "==title",
        "Quota Overview",
        "Quota",
        "Quota Ready Error",
        "profiles",
        "suffixprofiles",
        "profiles ",
        "PROFILE",
        "PROFILE names",
        "PROFILE Ready",
        "PROFILE Error",
        "Ready Error",
        "Ready thin",
        "healthy critical",
        "unhealthy",
        "HEALTHY",
        "ready",
        "blocked",
        "Blocked",
        "thin",
        "critical",
        "exhausted",
        "workspace: value",
        "  workspace: value",
        "\u{001c}workspace: value",
        "workspace: Ready",
        "error: value",
        "resets: value",
        "status: value",
        "reset credits: value",
        "press q",
        "  press q",
        "\u{2003}press q",
        "press Ready",
        "雪\0\n\"\\",
        "Status",
        "Status:",
        "Status:   ",
        "Status: Error: detail ",
        "status: thin",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    for label in [
        "Available",
        "Last Updated",
        "5h remaining pool",
        "Weekly remaining pool",
        "Remaining pool",
        "Account",
        "Plan",
        "Status",
        "Main",
        "5h",
        "Weekly",
        "Reset credits",
        "Auth",
        "Provider",
        "Reset",
        "Project",
        "Bucket",
        "Quota",
        "Rate groups",
        "Model groups",
        "Admin API",
        "Model provider",
        "Source",
    ] {
        for prefix in ["", "\u{2003}\t"] {
            for suffix in ["", "\u{00a0}\t 雪:more ", " Ready Error "] {
                cases.push(format!("{prefix}{label}:{suffix}"));
            }
        }
    }
    cases.push(format!("Status: {} ", "雪".repeat(4097)));
    let lines = cases
        .into_iter()
        .map(|line| {
            json!({
                "spans": format!("{:?}", quota_human_tui_spans(&line)),
                "cell_style": format!("{:?}", quota_watch_status_style(&line)),
                "line": line,
            })
        })
        .collect::<Vec<_>>();
    let texts = [
        "",
        "\n",
        "Status: Ready \nPROFILE\npress q\r\n",
        "\r\n\u{2003}Account: 雪 \r\n\n",
    ]
    .into_iter()
    .map(|text| {
        json!({
            "input": text, "text": format!("{:?}", quota_human_tui_text(text)),
        })
    })
    .collect::<Vec<_>>();
    json!({"lines": lines, "texts": texts})
}

#[test]
fn quota_tui_spans_preserve_original_complete_objects() {
    let corpus = quota_tui_corpus();
    let bytes = serde_json::to_vec(&corpus).unwrap();
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let observed = json!({
        "line_cases": corpus["lines"].as_array().unwrap().len(),
        "text_cases": corpus["texts"].as_array().unwrap().len(),
        "bytes": bytes.len(), "sha256": digest,
    });
    let expected: Value =
        serde_json::from_str(include_str!("span_presentation_baseline.json")).unwrap();
    assert_eq!(observed, expected);
}
