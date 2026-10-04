use super::*;
use serde_json::json;

#[test]
fn leak_filter_preserves_unmatched_text_and_scrubs_instruction_phrases() {
    let ordinary = " \nA normal response stays byte-for-byte intact. ";
    assert_eq!(
        gemini_provider_core_sanitize_internal_instruction_leak_text(ordinary).as_deref(),
        Some(ordinary)
    );

    for leaked in [
        "If a tool command succeeds, report the result.",
        "The report has a ## Verification section.",
        "The optimizer is active for every response.",
    ] {
        assert!(gemini_provider_core_internal_instruction_leak_text(leaked));
        assert_eq!(
            gemini_provider_core_sanitize_internal_instruction_leak_text(leaked),
            None
        );
    }
}

#[test]
fn leak_filter_keeps_only_safe_status_lines_with_unicode_trim() {
    let leaked =
        "\u{2003}If a tool command succeeds, report the result.\nVersi terbaru: 1.2.3\u{2003}";
    assert_eq!(
        gemini_provider_core_sanitize_internal_instruction_leak_text(leaked).as_deref(),
        Some("Versi terbaru: 1.2.3")
    );
}

#[test]
fn leak_filter_preserves_safe_paragraphs_and_joins_retained_status_lines() {
    let text = "Normal answer.\n\nIf a tool command succeeds, report the result.\r\nThe report includes ## Verification.\r\nVersi terbaru: 1.2.3\r\n\nAnother safe paragraph.";
    assert_eq!(
        gemini_provider_core_sanitize_internal_instruction_leak_text(text).as_deref(),
        Some("Normal answer.\n\nVersi terbaru: 1.2.3\n\nAnother safe paragraph.")
    );
}

#[test]
fn instruction_corpus_uses_system_messages_and_echo_checks_first_128_windows() {
    let corpus = gemini_provider_core_internal_instruction_corpus(&[
        json!({
            "role": "system",
            "content": [{"text": "Follow these exact instructions carefully and preserve every word."}],
        }),
        json!({
            "role": "user",
            "content": "This user text must not enter the corpus.",
        }),
    ]);
    assert_eq!(
        corpus,
        "follow these exact instructions carefully and preserve every word"
    );
    assert!(gemini_provider_core_text_echoes_internal_instruction(
        "FOLLOW, these exact instructions carefully and preserve every word!",
        &corpus,
    ));
    assert!(!gemini_provider_core_text_echoes_internal_instruction(
        "too few words here",
        &corpus,
    ));

    let mut words = (0..128)
        .map(|index| format!("prefix{index}"))
        .collect::<Vec<_>>();
    words.extend(
        [
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
        ]
        .map(str::to_string),
    );
    assert!(!gemini_provider_core_text_echoes_internal_instruction(
        &words.join(" "),
        "alpha bravo charlie delta echo foxtrot golf hotel",
    ));
}
