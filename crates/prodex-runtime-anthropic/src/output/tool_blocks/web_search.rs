use super::*;

pub fn runtime_anthropic_message_annotation_titles_by_url(
    output: &[serde_json::Value],
) -> BTreeMap<String, String> {
    let mut titles = BTreeMap::new();
    for item in output {
        let Some(parts) = item.get("content").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for part in parts {
            let Some(annotations) = part
                .get("annotations")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for annotation in annotations {
                let url = annotation
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        annotation
                            .get("url_citation")
                            .and_then(|value| value.get("url"))
                            .and_then(serde_json::Value::as_str)
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                let title = annotation
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        annotation
                            .get("url_citation")
                            .and_then(|value| value.get("title"))
                            .and_then(serde_json::Value::as_str)
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                if let (Some(url), Some(title)) = (url, title) {
                    titles
                        .entry(url.to_string())
                        .or_insert_with(|| title.to_string());
                }
            }
        }
    }
    titles
}

pub fn runtime_anthropic_web_search_blocks_from_output_item(
    item: &serde_json::Value,
    annotation_titles_by_url: &BTreeMap<String, String>,
) -> Vec<serde_json::Value> {
    let call_id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("web_search_call")
        .to_string();
    let queries = item
        .get("action")
        .and_then(|action| action.get("queries"))
        .and_then(serde_json::Value::as_array)
        .map(|queries| {
            queries
                .iter()
                .filter_map(|query| {
                    query
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| serde_json::Value::String(value.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let query = item
        .get("action")
        .and_then(|action| action.get("query"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            queries
                .first()
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();

    let mut seen_urls = BTreeSet::new();
    let mut results = Vec::new();
    if let Some(sources) = item
        .get("action")
        .and_then(|action| action.get("sources"))
        .and_then(serde_json::Value::as_array)
    {
        for source in sources {
            let Some(url) = source
                .get("url")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if !seen_urls.insert(url.to_string()) {
                continue;
            }
            let mut result = serde_json::Map::new();
            result.insert(
                "type".to_string(),
                serde_json::Value::String("web_search_result".to_string()),
            );
            result.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );
            if let Some(title) = source
                .get("title")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or_else(|| annotation_titles_by_url.get(url).map(String::as_str))
            {
                result.insert(
                    "title".to_string(),
                    serde_json::Value::String(title.to_string()),
                );
            }
            for key in [
                "encrypted_content",
                "page_age",
                "snippet",
                "summary",
                "text",
            ] {
                if let Some(value) = source
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    result.insert(
                        key.to_string(),
                        serde_json::Value::String(value.to_string()),
                    );
                }
            }
            results.push(serde_json::Value::Object(result));
        }
    }
    if results.is_empty() {
        for (url, title) in annotation_titles_by_url {
            if !seen_urls.insert(url.clone()) {
                continue;
            }
            results.push(serde_json::json!({
                "type": "web_search_result",
                "url": url,
                "title": title,
            }));
        }
    }

    vec![
        serde_json::json!({
            "type": "server_tool_use",
            "id": call_id,
            "name": "web_search",
            "input": {
                "query": query,
                "queries": queries,
            },
        }),
        serde_json::json!({
            "type": "web_search_tool_result",
            "tool_use_id": call_id,
            "content": results,
        }),
    ]
}
