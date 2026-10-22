use super::runtime_launch_dry_run_tui_text;
use prodex_mojo_core::launch::runtime_launch_tui_height;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

fn dry_run_tui_corpus() -> Value {
    let mut cases = [
        "",
        "\n",
        "plain",
        "Header:",
        "Label: disabled",
        "Proxy: disabled enabled",
        "Proxy: unknown",
        "Proxy: disabled:",
        "BINARY: enabled",
        "Binary: opaque",
        "CODEX_HOME: value",
        "Command: removed",
        "Model: token",
        "Ready",
        "Label:\u{2003}雪  ",
        "\u{2003}Label:\t 雪",
        "Label:\u{001c}雪",
        "Label:\u{200b}雪",
        "Label: x:y",
        "Label: x:y:",
        "Unknown: foo",
        "pRoXy: any",
        "bInArY: OpAqUe",
        "Binary: ready removed",
        "Unknown: REMOVED ready",
        "Label: \0\n雪\"\\",
        "\u{2003}COMMand:\u{3000}value  ",
        "a\r\nb\r\nc\r\nd\r\n",
        "\r\r",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    for rows in 0..=30 {
        cases.push("line\n".repeat(rows));
        cases.push(format!("{}tail", "line\n".repeat(rows)));
    }
    cases.push(format!("Unknown: {} ", "雪".repeat(4097)));
    Value::Array(
        cases
            .into_iter()
            .map(|input| {
                json!({
                    "height": runtime_launch_tui_height(&input).unwrap(),
                    "text": format!("{:?}", runtime_launch_dry_run_tui_text(&input).unwrap()),
                    "input": input,
                })
            })
            .collect(),
    )
}

#[test]
fn dry_run_tui_preserves_original_complete_text_and_heights() {
    let corpus = dry_run_tui_corpus();
    let bytes = serde_json::to_vec(&corpus).unwrap();
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let actual =
        json!({"cases": corpus.as_array().unwrap().len(), "bytes": bytes.len(), "sha256": digest});
    let expected: Value = serde_json::from_str(include_str!(
        "child_process_dry_run_presentation_baseline.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
}
