//! Presidio analyzer language selection and result merge helpers.

use super::findings::PresidioAnalyzerResult;

pub(super) fn detect_presidio_language(text: &str, candidates: &[String]) -> Option<String> {
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }

    let id_keywords = [
        "yang", "dan", "di", "ke", "dari", "saya", "kami", "anda", "nomor", "nama", "alamat",
        "tanggal", "lahir", "dengan", "untuk",
    ];
    let en_keywords = [
        "the", "and", "to", "from", "my", "name", "phone", "email", "address", "with", "for",
        "birth",
    ];

    let lower_text = text.to_lowercase();
    let id_score = id_keywords
        .iter()
        .filter(|keyword| lower_text.contains(*keyword))
        .count();
    let en_score = en_keywords
        .iter()
        .filter(|keyword| lower_text.contains(*keyword))
        .count();

    if id_score > en_score && candidates.contains(&"id".to_string()) {
        Some("id".to_string())
    } else if en_score > id_score && candidates.contains(&"en".to_string()) {
        Some("en".to_string())
    } else {
        candidates.first().cloned()
    }
}

pub(super) fn merge_presidio_analyzer_results(
    mut results: Vec<PresidioAnalyzerResult>,
) -> Vec<PresidioAnalyzerResult> {
    results.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| a.end.cmp(&b.end))
            .then_with(|| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| b.entity_type.cmp(&a.entity_type))
    });

    let mut merged: Vec<PresidioAnalyzerResult> = Vec::new();
    for result in results {
        if let Some(last) = merged.last_mut() {
            if last.start == result.start
                && last.end == result.end
                && last.entity_type == result.entity_type
            {
                if result.score > last.score {
                    *last = result;
                }
                continue;
            }

            let overlaps = result.start < last.end && result.end > last.start;
            if overlaps
                && (result.score > last.score
                    || (result.score == last.score
                        && (result.end - result.start) > (last.end - last.start)))
            {
                if (result.start >= last.start && result.end <= last.end)
                    || (last.start >= result.start && last.end <= result.end)
                {
                    if result.score > last.score {
                        *last = result;
                    }
                    continue;
                } else if result.score > last.score {
                    last.start = last.start.min(result.start);
                    last.end = last.end.max(result.end);
                    last.score = result.score;
                    last.entity_type = result.entity_type;
                    last.language = result.language;
                    continue;
                }
            }
        }
        merged.push(result);
    }
    merged
}
