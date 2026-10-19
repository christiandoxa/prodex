use super::*;

fn finding(
    start: usize,
    end: usize,
    score: f64,
    entity: &str,
    language: &str,
) -> PresidioAnalyzerResult {
    PresidioAnalyzerResult {
        start,
        end,
        score,
        entity_type: entity.to_string(),
        language: language.to_string(),
    }
}

fn assert_findings(actual: Vec<PresidioAnalyzerResult>, expected: Vec<PresidioAnalyzerResult>) {
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

#[test]
fn compiled_analyzer_merge_preserves_duplicate_score_entity_and_language_order() {
    assert_findings(
        merge_presidio_analyzer_results(vec![
            finding(8, 10, 0.7, "EMAIL_ADDRESS", "en"),
            finding(0, 4, 0.8, "PERSON", "id"),
            finding(0, 4, 0.9, "PERSON", "en"),
            finding(0, 4, 0.9, "PERSON", "id"),
            finding(0, 4, 0.9, "EMAIL_ADDRESS", "en"),
        ])
        .unwrap(),
        vec![
            finding(0, 4, 0.9, "PERSON", "en"),
            finding(0, 4, 0.9, "EMAIL_ADDRESS", "en"),
            finding(0, 4, 0.8, "PERSON", "id"),
            finding(8, 10, 0.7, "EMAIL_ADDRESS", "en"),
        ],
    );
}

#[test]
fn compiled_analyzer_merge_preserves_adjacent_overlap_and_containment_contract() {
    for (input, expected) in [
        (
            vec![
                finding(0, 5, 0.7, "PERSON", "en"),
                finding(3, 8, 0.9, "EMAIL_ADDRESS", "id"),
            ],
            vec![finding(0, 8, 0.9, "EMAIL_ADDRESS", "id")],
        ),
        (
            vec![
                finding(0, 5, 0.9, "PERSON", "en"),
                finding(3, 8, 0.7, "EMAIL_ADDRESS", "id"),
            ],
            vec![
                finding(0, 5, 0.9, "PERSON", "en"),
                finding(3, 8, 0.7, "EMAIL_ADDRESS", "id"),
            ],
        ),
        (
            vec![
                finding(0, 10, 0.7, "PERSON", "en"),
                finding(3, 5, 0.9, "EMAIL_ADDRESS", "id"),
            ],
            vec![finding(3, 5, 0.9, "EMAIL_ADDRESS", "id")],
        ),
        (
            vec![
                finding(0, 5, 0.8, "PERSON", "en"),
                finding(3, 10, 0.8, "EMAIL_ADDRESS", "id"),
            ],
            vec![
                finding(0, 5, 0.8, "PERSON", "en"),
                finding(3, 10, 0.8, "EMAIL_ADDRESS", "id"),
            ],
        ),
        (
            vec![
                finding(0, 5, 0.8, "PERSON", "en"),
                finding(0, 10, 0.8, "EMAIL_ADDRESS", "id"),
            ],
            vec![finding(0, 5, 0.8, "PERSON", "en")],
        ),
        (
            vec![
                finding(0, 5, 0.8, "PERSON", "en"),
                finding(5, 10, 0.9, "EMAIL_ADDRESS", "id"),
            ],
            vec![
                finding(0, 5, 0.8, "PERSON", "en"),
                finding(5, 10, 0.9, "EMAIL_ADDRESS", "id"),
            ],
        ),
        (
            vec![
                finding(0, 4, 0.1, "PERSON", "en"),
                finding(3, 8, 0.5, "EMAIL_ADDRESS", "id"),
                finding(6, 10, 0.9, "API_KEY", "en"),
            ],
            vec![finding(0, 10, 0.9, "API_KEY", "en")],
        ),
    ] {
        assert_findings(merge_presidio_analyzer_results(input).unwrap(), expected);
    }
}

#[test]
fn compiled_analyzer_merge_handles_large_reverse_order_without_reordering_ties() {
    let input = (0..4096)
        .rev()
        .map(|i| finding(i * 2, i * 2 + 1, 0.5, "PERSON", "id"))
        .collect();
    let expected = (0..4096)
        .map(|i| finding(i * 2, i * 2 + 1, 0.5, "PERSON", "id"))
        .collect();
    assert_findings(merge_presidio_analyzer_results(input).unwrap(), expected);
}

#[test]
fn compiled_presidio_language_caller_preserves_substrings_ties_and_candidate_priority() {
    for (text, candidates, expected) in [
        ("NAMA DAN ALAMAT SAYA", vec!["en", "id"], Some("id")),
        ("my name and phone address", vec!["id", "en"], Some("en")),
        ("dan and", vec!["en", "id"], Some("en")),
        ("dan and", vec!["id", "en"], Some("id")),
        ("dan dan dan and the", vec!["id", "en"], Some("en")),
        ("dan", vec!["fr", "en"], Some("fr")),
        ("Dİ", vec!["en", "id"], Some("id")),
        ("KE", vec!["en", "id"], Some("id")),
        ("İD", vec!["en", "id"], Some("en")),
        ("🧭東京", vec!["id", "en"], Some("id")),
        ("phone", vec!["fr"], Some("fr")),
        ("phone", vec![], None),
    ] {
        let candidates = candidates
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            detect_presidio_language(text, &candidates)
                .unwrap()
                .as_deref(),
            expected,
            "{text}"
        );
    }
}
