use super::*;
use serde_json::Value;

const STRUCTURED_JSON_SUMMARY_MAX_VISITED_NODES: usize = 2048;
const STRUCTURED_JSON_SUMMARY_MAX_DEPTH: usize = 8;
const STRUCTURED_JSON_SUMMARY_MAX_SAMPLES: usize = 8;
const STRUCTURED_JSON_SUMMARY_MAX_KEYS: usize = 16;
const STRUCTURED_JSON_NDJSON_MIN_RECORDS: usize = 2;

#[derive(Debug, Default)]
struct StructuredJsonSummary {
    visited_nodes: usize,
    object_count: usize,
    array_count: usize,
    scalar_count: usize,
    key_counts: BTreeMap<String, usize>,
    level_counts: BTreeMap<String, usize>,
    error_samples: Vec<String>,
    path_samples: Vec<String>,
    id_samples: Vec<String>,
}

pub(super) fn compact_structured_json_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> Option<String> {
    compact_whole_json_output(input, options).or_else(|| compact_ndjson_output(input, options))
}

fn compact_whole_json_output(input: &str, options: &CommandOutputCompactOptions) -> Option<String> {
    let trimmed = input.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }

    let value = serde_json::from_str::<Value>(trimmed).ok()?;
    let mut summary = StructuredJsonSummary::default();
    summarize_structured_json_value(&value, 0, &mut summary);

    let mut output = Vec::new();
    output.push(format!("pcs: json ({}->sum)", count_text_lines(input)));
    output.push(match &value {
        Value::Object(map) => format!("shape: object keys={}", map.len()),
        Value::Array(values) => format!("shape: array items={}", values.len()),
        Value::String(_) => "shape: string".to_string(),
        Value::Number(_) => "shape: number".to_string(),
        Value::Bool(_) => "shape: bool".to_string(),
        Value::Null => "shape: null".to_string(),
    });
    push_structured_json_summary_lines(&mut output, &summary, options);
    finalize_structured_json_compaction(input, output, options)
}

fn compact_ndjson_output(input: &str, options: &CommandOutputCompactOptions) -> Option<String> {
    let lines = command_lines(input);
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    if non_empty < STRUCTURED_JSON_NDJSON_MIN_RECORDS {
        return None;
    }

    let mut parsed = 0usize;
    let mut summary = StructuredJsonSummary::default();
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        parsed = parsed.saturating_add(1);
        summarize_structured_json_value(&value, 0, &mut summary);
    }

    if parsed < STRUCTURED_JSON_NDJSON_MIN_RECORDS || parsed.saturating_mul(2) < non_empty {
        return None;
    }

    let mut output = Vec::new();
    output.push(format!("pcs: ndjson ({}->sum)", count_text_lines(input)));
    output.push(format!(
        "sum: ndjson records={}, non_json={}",
        parsed,
        non_empty.saturating_sub(parsed)
    ));
    push_structured_json_summary_lines(&mut output, &summary, options);
    finalize_structured_json_compaction(input, output, options)
}

fn summarize_structured_json_value(
    value: &Value,
    depth: usize,
    summary: &mut StructuredJsonSummary,
) {
    if summary.visited_nodes >= STRUCTURED_JSON_SUMMARY_MAX_VISITED_NODES
        || depth > STRUCTURED_JSON_SUMMARY_MAX_DEPTH
    {
        return;
    }
    summary.visited_nodes = summary.visited_nodes.saturating_add(1);

    match value {
        Value::Object(map) => {
            summary.object_count = summary.object_count.saturating_add(1);
            for (key, child) in map {
                increment_count(&mut summary.key_counts, key);
                collect_structured_json_key_sample(key, child, summary);
                summarize_structured_json_value(child, depth.saturating_add(1), summary);
            }
        }
        Value::Array(values) => {
            summary.array_count = summary.array_count.saturating_add(1);
            for child in values {
                summarize_structured_json_value(child, depth.saturating_add(1), summary);
                if summary.visited_nodes >= STRUCTURED_JSON_SUMMARY_MAX_VISITED_NODES {
                    break;
                }
            }
        }
        _ => {
            summary.scalar_count = summary.scalar_count.saturating_add(1);
        }
    }
}

fn collect_structured_json_key_sample(
    key: &str,
    value: &Value,
    summary: &mut StructuredJsonSummary,
) {
    let lower = key.to_ascii_lowercase();
    if matches!(lower.as_str(), "level" | "severity" | "status" | "state")
        && let Some(label) = structured_json_scalar_string(value)
    {
        let label = label.to_ascii_lowercase();
        increment_count(&mut summary.level_counts, &label);
        if matches!(label.as_str(), "error" | "fatal" | "failed" | "failure") {
            let sample = format!("error sample: {key}={label}");
            push_structured_json_sample(&mut summary.error_samples, sample);
        }
    }

    if structured_json_key_is_error_like(&lower) {
        let sample = format!(
            "error sample: \"{}\"={}",
            key,
            structured_json_value_sample(value)
        );
        push_structured_json_sample(&mut summary.error_samples, sample);
    }

    if structured_json_key_is_path_like(&lower)
        && let Some(label) = structured_json_scalar_string(value)
    {
        let sample = format!("{key}={label}");
        push_structured_json_sample(&mut summary.path_samples, sample);
    }

    if structured_json_key_is_id_like(&lower)
        && let Some(label) = structured_json_scalar_string(value)
    {
        let sample = format!("{key}={label}");
        push_structured_json_sample(&mut summary.id_samples, sample);
    }
}

fn push_structured_json_summary_lines(
    output: &mut Vec<String>,
    summary: &StructuredJsonSummary,
    options: &CommandOutputCompactOptions,
) {
    output.push(format!(
        "nodes: objects={}, arrays={}, scalars={}, visited={}",
        summary.object_count, summary.array_count, summary.scalar_count, summary.visited_nodes,
    ));
    if !summary.key_counts.is_empty() {
        output.push(format_count_map(
            "keys",
            &summary.key_counts,
            STRUCTURED_JSON_SUMMARY_MAX_KEYS,
        ));
    }
    if !summary.level_counts.is_empty() {
        output.push(format_count_map("levels", &summary.level_counts, 10));
    }
    push_structured_json_sample_lines(
        output,
        "error samples",
        &summary.error_samples,
        options.max_line_chars,
    );
    push_structured_json_sample_lines(
        output,
        "path samples",
        &summary.path_samples,
        options.max_line_chars,
    );
    push_structured_json_sample_lines(
        output,
        "id samples",
        &summary.id_samples,
        options.max_line_chars,
    );
}

fn push_structured_json_sample_lines(
    output: &mut Vec<String>,
    label: &str,
    samples: &[String],
    max_line_chars: usize,
) {
    if samples.is_empty() {
        return;
    }
    output.push(format!("{label}:"));
    for sample in samples.iter().take(STRUCTURED_JSON_SUMMARY_MAX_SAMPLES) {
        output.push(format!(
            "  {}",
            truncate_command_line(sample, max_line_chars)
        ));
    }
    if samples.len() > STRUCTURED_JSON_SUMMARY_MAX_SAMPLES {
        output.push(format!(
            "  [... {} more samples ...]",
            samples.len() - STRUCTURED_JSON_SUMMARY_MAX_SAMPLES,
        ));
    }
}

fn finalize_structured_json_compaction(
    original: &str,
    lines: Vec<String>,
    options: &CommandOutputCompactOptions,
) -> Option<String> {
    let candidate = lines_to_text(lines);
    let candidate =
        ensure_no_critical_signal_loss_for_structured_json(original, &candidate, options);
    if candidate.len() < original.len() && critical_signal_self_check(original, &candidate).passed()
    {
        Some(candidate)
    } else {
        None
    }
}

fn ensure_no_critical_signal_loss_for_structured_json(
    original: &str,
    candidate: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    if critical_signal_self_check(original, candidate).passed() {
        return candidate.to_string();
    }

    let mut output = command_lines(candidate)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    output.push("critical signal rescue:".to_string());
    for line in command_lines(original) {
        if !is_critical_preserve_line(line) {
            continue;
        }
        output.push(truncate_command_line(line.trim(), options.max_line_chars));
        let text = lines_to_text(output.clone());
        if critical_signal_self_check(original, &text).passed() {
            return text;
        }
    }

    lines_to_text(output)
}

fn structured_json_scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => Some("null".to_string()),
        _ => None,
    }
}

fn structured_json_value_sample(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "null".to_string(),
        Value::Object(map) => {
            let mut parts = Vec::new();
            for key in [
                "code", "type", "kind", "status", "message", "path", "file", "id",
            ] {
                if let Some(value) = map.get(key).and_then(structured_json_scalar_string) {
                    parts.push(format!("{key}={value}"));
                }
            }
            if parts.is_empty() {
                format!("object(keys={})", map.len())
            } else {
                parts.join(", ")
            }
        }
        Value::Array(values) => format!("array(items={})", values.len()),
    }
}

fn structured_json_key_is_error_like(lower: &str) -> bool {
    matches!(
        lower,
        "error" | "errors" | "err" | "exception" | "failure" | "failures" | "fatal"
    )
}

fn structured_json_key_is_path_like(lower: &str) -> bool {
    matches!(
        lower,
        "path" | "file" | "filename" | "filepath" | "source" | "target" | "uri" | "url" | "cwd"
    )
}

fn structured_json_key_is_id_like(lower: &str) -> bool {
    lower == "id" || lower.ends_with("_id") || lower.ends_with("id")
}

fn push_structured_json_sample(samples: &mut Vec<String>, sample: String) {
    if samples.len() >= STRUCTURED_JSON_SUMMARY_MAX_SAMPLES
        || samples.iter().any(|existing| existing == &sample)
    {
        return;
    }
    samples.push(sample);
}

fn increment_count(counts: &mut BTreeMap<String, usize>, key: &str) {
    counts
        .entry(key.to_string())
        .and_modify(|count| *count = count.saturating_add(1))
        .or_insert(1);
}
