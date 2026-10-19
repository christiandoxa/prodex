//! Presidio analyzer language selection and result merge helpers.

use super::findings::PresidioAnalyzerResult;

pub(super) fn detect_presidio_language(
    text: &str,
    candidates: &[String],
) -> anyhow::Result<Option<String>> {
    let selected =
        prodex_mojo_core::redaction::presidio_language_plan(text, candidates).map_err(|error| {
            anyhow::anyhow!("compiled Mojo Presidio language selection failed: {error:?}")
        })?;
    Ok(selected.map(|index| candidates[index].clone()))
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
