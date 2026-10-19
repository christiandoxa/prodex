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
    results: Vec<PresidioAnalyzerResult>,
) -> anyhow::Result<Vec<PresidioAnalyzerResult>> {
    let facts = results
        .iter()
        .map(|result| prodex_mojo_core::redaction::AnalyzerFinding {
            start: result.start,
            end: result.end,
            score: result.score,
            entity: &result.entity_type,
        })
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::redaction::analyzer_merge_plan(&facts).map_err(|error| {
        anyhow::anyhow!("compiled Mojo Presidio analyzer merge failed: {error:?}")
    })?;
    Ok(plan
        .into_iter()
        .map(|selected| {
            let mut result = results[selected.source as usize].clone();
            result.start = selected.start as usize;
            result.end = selected.end as usize;
            result.score = selected.score;
            result
        })
        .collect())
}

#[cfg(test)]
#[path = "analyzer_tests.rs"]
mod tests;
