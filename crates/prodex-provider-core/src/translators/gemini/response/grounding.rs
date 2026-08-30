//! Gemini grounding, citations, and web-search response items.

use serde_json::{Value, json};

use super::response_status::gemini_finish_reason;

pub(crate) fn gemini_citation_text(value: &Value) -> Option<String> {
    gemini_finish_reason(value)?;
    let citations = value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("citationMetadata"))
        .and_then(|metadata| metadata.get("citations"))
        .and_then(Value::as_array)?;
    let mut lines = citations
        .iter()
        .filter_map(|citation| {
            let uri = citation
                .get("uri")
                .and_then(Value::as_str)
                .filter(|uri| !uri.trim().is_empty())?;
            let title = citation
                .get("title")
                .and_then(Value::as_str)
                .filter(|title| !title.trim().is_empty());
            Some(match title {
                Some(title) => format!("({title}) {uri}"),
                None => uri.to_string(),
            })
        })
        .collect::<Vec<_>>();
    lines.sort();
    lines.dedup();
    if lines.is_empty() {
        return None;
    }
    #[cfg(feature = "mojo")]
    {
        let lines = lines.join("\n");
        let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
            prodex_mojo_core::rich::GeminiResponseKernelOperation::CitationText,
        );
        input.delta = Some(&lines);
        super::super::stream::gemini_mojo_value(input)
            .as_str()
            .map(str::to_string)
    }
    #[cfg(not(feature = "mojo"))]
    Some(format!("Citations:\n{}", lines.join("\n")))
}

pub(crate) fn gemini_web_search_call_from_grounding(
    value: &Value,
    response_id: &str,
) -> Option<Value> {
    let candidate = value.get("candidates")?.as_array()?.first()?;
    let mut sources = Vec::new();
    let grounding_metadata = candidate.get("groundingMetadata");
    gemini_collect_grounding_chunk_sources(grounding_metadata, &mut sources);
    gemini_collect_citation_sources(candidate, &mut sources);
    gemini_collect_url_metadata_sources(candidate, &mut sources);
    let queries = grounding_metadata
        .and_then(|metadata| metadata.get("webSearchQueries"))
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    if sources.is_empty() && queries.is_empty() {
        return None;
    }
    #[cfg(feature = "mojo")]
    {
        let queries_json = serde_json::to_string(&queries).expect("Gemini queries serialize");
        let sources_json = serde_json::to_string(&sources).expect("Gemini sources serialize");
        let first_url = queries.is_empty().then(|| {
            sources
                .first()
                .and_then(|source| source.get("url"))
                .and_then(Value::as_str)
        });
        let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
            prodex_mojo_core::rich::GeminiResponseKernelOperation::WebSearchCall,
        );
        input.response_id = Some(response_id);
        input.content = Some(&queries_json);
        input.output = Some(&sources_json);
        input.delta = first_url.flatten();
        Some(super::super::stream::gemini_mojo_value(input))
    }
    #[cfg(not(feature = "mojo"))]
    {
        let action = if queries.is_empty() {
            let first_url = sources
                .first()
                .and_then(|source| source.get("url"))
                .and_then(Value::as_str)?;
            json!({
                "type": "open_page",
                "url": first_url,
                "sources": sources,
            })
        } else {
            json!({
                "type": "search",
                "queries": queries,
                "sources": sources,
            })
        };
        Some(json!({
            "type": "web_search_call",
            "id": format!("ws_{response_id}"),
            "status": "completed",
            "action": action,
        }))
    }
}

fn gemini_collect_grounding_chunk_sources(
    grounding_metadata: Option<&Value>,
    sources: &mut Vec<Value>,
) {
    let Some(chunks) = grounding_metadata
        .and_then(|metadata| metadata.get("groundingChunks"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for chunk in chunks {
        for source_kind in ["web", "retrievedContext"] {
            if let Some(source) = chunk
                .get(source_kind)
                .and_then(gemini_url_source_from_metadata)
            {
                gemini_push_unique_url_source(sources, source);
            }
        }
    }
}

fn gemini_collect_citation_sources(candidate: &Value, sources: &mut Vec<Value>) {
    let Some(citations) = candidate
        .get("citationMetadata")
        .and_then(|metadata| {
            metadata
                .get("citations")
                .or_else(|| metadata.get("citationSources"))
        })
        .and_then(Value::as_array)
    else {
        return;
    };
    for citation in citations {
        if let Some(source) = gemini_url_source_from_metadata(citation) {
            gemini_push_unique_url_source(sources, source);
        }
    }
}

fn gemini_collect_url_metadata_sources(candidate: &Value, sources: &mut Vec<Value>) {
    let Some(url_metadata) = candidate
        .get("urlContextMetadata")
        .and_then(|metadata| {
            metadata
                .get("urlMetadata")
                .or_else(|| metadata.get("url_metadata"))
        })
        .and_then(Value::as_array)
    else {
        return;
    };
    for entry in url_metadata {
        if let Some(source) = gemini_url_source_from_metadata(entry) {
            gemini_push_unique_url_source(sources, source);
        }
    }
}

fn gemini_url_source_from_metadata(value: &Value) -> Option<Value> {
    let uri = ["uri", "url", "retrievedUrl", "retrieved_url"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .filter(|uri| !uri.trim().is_empty())?;
    let mut source = json!({
        "type": "url",
        "url": uri,
    });
    if let Some(title) = value
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
    {
        source["title"] = Value::String(title.to_string());
    }
    if let Some(status) = value
        .get("urlRetrievalStatus")
        .or_else(|| value.get("url_retrieval_status"))
        .or_else(|| value.get("status"))
        .filter(|status| !status.is_null())
    {
        source["status"] = status.clone();
    }
    Some(source)
}

fn gemini_push_unique_url_source(sources: &mut Vec<Value>, source: Value) {
    let source_url = source.get("url").and_then(Value::as_str);
    if sources
        .iter()
        .any(|existing| existing.get("url").and_then(Value::as_str) == source_url)
    {
        return;
    }
    sources.push(source);
}
