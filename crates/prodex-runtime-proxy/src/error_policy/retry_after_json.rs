//! Mechanical serde-tree and HTTP-date decoding; retry policy lives in Mojo.
use prodex_mojo_core::json::{JsonKind, JsonNode, runtime_retry_after_json};
use serde_json::Value;
use std::{
    borrow::Cow,
    time::{Duration, SystemTime},
};

struct Node<'a> {
    kind: JsonKind,
    first: Option<usize>,
    next: Option<usize>,
    parent: Option<usize>,
    key: &'a str,
    text: Cow<'a, str>,
}

fn append<'a>(
    value: &'a Value,
    key: &'a str,
    parent: Option<usize>,
    depth: usize,
    nodes: &mut Vec<Node<'a>>,
) -> Option<usize> {
    if depth > 128 || nodes.len() >= 65_537 {
        return None;
    }
    let (kind, text) = match value {
        Value::Null => (JsonKind::Null, Cow::Borrowed("")),
        Value::Bool(false) => (JsonKind::False, Cow::Borrowed("false")),
        Value::Bool(true) => (JsonKind::True, Cow::Borrowed("true")),
        Value::Number(n) => (JsonKind::Number, Cow::Owned(n.to_string())),
        Value::String(text) => (JsonKind::String, Cow::Borrowed(text.as_str())),
        Value::Array(_) => (JsonKind::Array, Cow::Borrowed("")),
        Value::Object(_) => (JsonKind::Object, Cow::Borrowed("")),
    };
    let index = nodes.len();
    nodes.push(Node {
        kind,
        first: None,
        next: None,
        parent,
        key,
        text,
    });
    let mut previous: Option<usize> = None;
    let mut child = |key, value| -> Option<()> {
        let current = append(value, key, Some(index), depth + 1, nodes)?;
        if let Some(previous) = previous {
            nodes[previous].next = Some(current);
        } else {
            nodes[index].first = Some(current);
        }
        previous = Some(current);
        Some(())
    };
    match value {
        Value::Array(values) => {
            for value in values {
                child("", value)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                child(key, value)?;
            }
        }
        _ => {}
    }
    Some(index)
}

pub(super) fn retry_after(value: &Value, fallback: Option<Duration>) -> Option<Duration> {
    // Capture wall time once before decoding, never once per candidate header.
    let now = SystemTime::now();
    let mut owned = Vec::new();
    if append(value, "", None, 0, &mut owned).is_none() {
        return fallback;
    }
    let nodes: Vec<_> = owned
        .iter()
        .map(|node| JsonNode {
            kind: node.kind,
            first_child: node.first,
            next_sibling: node.next,
            parent: node.parent,
            key: node.key,
            text: &node.text,
            raw_start: 0,
            raw_length: 0,
        })
        .collect();
    // This is a generic HTTP-date codec, not header selection or retry policy.
    let dates: Vec<_> = owned
        .iter()
        .map(|node| decode_http_date_millis(&node.text, now))
        .collect();
    let fallback_millis = fallback
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(-1);
    runtime_retry_after_json(&nodes, &dates, fallback_millis)
        .expect("Mojo structured retry-advice planner returned invalid output")
        .map(Duration::from_millis)
}

fn decode_http_date_millis(text: &str, now: SystemTime) -> i64 {
    httpdate::parse_http_date(text.trim())
        .ok()
        .map(|date| {
            // Round up so conversion to the millisecond ABI cannot advance a deadline.
            let millis = date
                .duration_since(now)
                .unwrap_or_default()
                .as_nanos()
                .div_ceil(1_000_000);
            i64::try_from(millis).unwrap_or(i64::MAX)
        })
        .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_codec_rounds_up_without_restarting_or_advancing_the_deadline() {
        let deadline = std::time::UNIX_EPOCH + Duration::from_secs(100);
        let header = httpdate::fmt_http_date(deadline);
        // Sub-millisecond values representable on Windows' 100ns clock too.
        assert_eq!(
            decode_http_date_millis(&header, deadline - Duration::from_micros(1)),
            1
        );
        assert_eq!(
            decode_http_date_millis(&header, deadline - Duration::from_micros(999_999)),
            1000
        );
        assert_eq!(
            decode_http_date_millis(&header, deadline + Duration::from_secs(1)),
            0
        );
        assert_eq!(decode_http_date_millis("invalid", deadline), -1);
    }
}
