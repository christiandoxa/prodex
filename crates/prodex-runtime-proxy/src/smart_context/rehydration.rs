use super::*;
use std::borrow::Cow;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextExactAppendixRange {
    pub reference: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SmartContextAppendixLineRange {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SmartContextSeenExactAppendixBody {
    body: String,
    refs: Vec<String>,
}

pub fn smart_context_render_exact_appendix(
    label: &str,
    ranges: Vec<SmartContextExactAppendixRange>,
) -> Option<(String, usize)> {
    let range_count = ranges
        .iter()
        .filter(|range| !range.body.trim().is_empty())
        .count();
    if range_count == 0 {
        return None;
    }
    let mut rendered = vec![label.to_string()];
    let mut seen = BTreeMap::<(String, usize), Vec<SmartContextSeenExactAppendixBody>>::new();
    for range in smart_context_merge_exact_appendix_ranges(ranges) {
        if range.body.trim().is_empty() {
            continue;
        }
        rendered.push(smart_context_render_exact_appendix_range(&range, &mut seen));
    }
    Some((rendered.join("\n"), range_count))
}

fn smart_context_merge_exact_appendix_ranges(
    ranges: Vec<SmartContextExactAppendixRange>,
) -> Vec<SmartContextExactAppendixRange> {
    let mut merged = Vec::<SmartContextExactAppendixRange>::new();
    for range in ranges {
        let Some((range_base, range_lines)) =
            smart_context_parse_exact_appendix_line_ref(&range.reference)
        else {
            merged.push(range);
            continue;
        };
        let Some(last) = merged.last_mut() else {
            merged.push(range);
            continue;
        };
        let Some((last_base, last_lines)) =
            smart_context_parse_exact_appendix_line_ref(&last.reference)
        else {
            merged.push(range);
            continue;
        };
        if last_base != range_base
            || range_lines.start > last_lines.end.saturating_add(1)
            || range_lines.end < last_lines.start
        {
            merged.push(range);
            continue;
        }

        let overlap = if range_lines.start <= last_lines.end {
            last_lines
                .end
                .saturating_sub(range_lines.start)
                .saturating_add(1)
        } else {
            0
        };
        let next_line_count = range.body.lines().count();
        if overlap >= next_line_count && range_lines.end > last_lines.end {
            merged.push(range);
            continue;
        }
        let next_lines = range.body.lines().collect::<Vec<_>>();
        if overlap < next_lines.len() {
            if !last.body.is_empty() {
                last.body.push('\n');
            }
            last.body.push_str(&next_lines[overlap..].join("\n"));
        }
        let end = last_lines.end.max(range_lines.end);
        last.reference = format!("{last_base}#L{}-L{end}", last_lines.start);
    }
    merged
}

fn smart_context_parse_exact_appendix_line_ref(
    reference: &str,
) -> Option<(String, SmartContextAppendixLineRange)> {
    let (base, range) = reference.rsplit_once("#L")?;
    let (start, end) = range.split_once("-L")?;
    Some((
        base.to_string(),
        SmartContextAppendixLineRange {
            start: start.parse().ok()?,
            end: end.parse().ok()?,
        },
    ))
}

pub fn smart_context_compact_line_refs_if_shorter(refs: &[String]) -> String {
    let joined = refs.join(",");
    let Some((base, ranges)) = smart_context_line_refs_same_base(refs) else {
        return joined;
    };
    if ranges.len() < 2 {
        return joined;
    }
    let compact = format!(
        "{base}#{}",
        ranges
            .iter()
            .map(|range| format!("L{}-L{}", range.start, range.end))
            .collect::<Vec<_>>()
            .join(",")
    );
    if compact.len() < joined.len()
        && smart_context_non_alias_artifact_reference_line_range_count(&compact)
            .is_some_and(|range_count| range_count == ranges.len())
    {
        compact
    } else {
        joined
    }
}

fn smart_context_line_refs_same_base(
    refs: &[String],
) -> Option<(String, Vec<SmartContextAppendixLineRange>)> {
    let mut base: Option<String> = None;
    let mut ranges = Vec::new();
    for reference in refs {
        let (next_base, range) = smart_context_parse_exact_appendix_line_ref(reference)?;
        if let Some(base) = base.as_ref() {
            if base != &next_base {
                return None;
            }
        } else {
            base = Some(next_base);
        }
        ranges.push(range);
    }
    Some((base?, ranges))
}

fn smart_context_render_exact_appendix_range(
    range: &SmartContextExactAppendixRange,
    seen: &mut BTreeMap<(String, usize), Vec<SmartContextSeenExactAppendixBody>>,
) -> String {
    let content_hash = smart_context_hash_text(&range.body);
    let byte_len = range.body.len();
    let key = (content_hash.clone(), byte_len);
    if let Some(entries) = seen.get_mut(&key)
        && let Some(existing) = entries.iter_mut().find(|entry| entry.body == range.body)
    {
        let marker = format!(
            "[psc exdup h={content_hash} b={byte_len} refs={}]",
            smart_context_compact_line_refs_if_shorter(&existing.refs)
        );
        let candidate = format!("{}\n{marker}", range.reference);
        let original = format!("{}\n{}", range.reference, range.body);
        existing.refs.push(range.reference.clone());
        if candidate.len() < original.len() {
            return candidate;
        }
        return original;
    }
    seen.entry(key)
        .or_default()
        .push(SmartContextSeenExactAppendixBody {
            body: range.body.clone(),
            refs: vec![range.reference.clone()],
        });
    format!("{}\n{}", range.reference, range.body)
}

fn smart_context_non_alias_artifact_reference_line_range_count(token: &str) -> Option<usize> {
    let token = smart_context_trim_artifact_ref_token(token);
    let raw = if let Some(raw) = token.strip_prefix("prodex-artifact:") {
        Cow::Borrowed(raw)
    } else if let Some(raw) = token.strip_prefix(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX) {
        if raw.starts_with("sc:") {
            Cow::Borrowed(raw)
        } else {
            Cow::Owned(format!("sc:{raw}"))
        }
    } else {
        Cow::Borrowed(token)
    };
    let raw = raw.as_ref();
    if !raw.starts_with("sc:") {
        return None;
    }

    let mut id_end = 3usize;
    for (offset, ch) in raw[3..].char_indices() {
        if ch.is_ascii_hexdigit() {
            id_end = 3 + offset + ch.len_utf8();
        } else {
            break;
        }
    }
    if id_end == 3 {
        return None;
    }

    Some(smart_context_parse_line_ranges(&raw[id_end..]).len())
}

fn smart_context_trim_artifact_ref_token(token: &str) -> &str {
    token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | '`'
                | ':'
                | ';'
                | '.'
                | ','
                | '!'
                | '?'
                | '('
                | '['
                | '{'
                | '<'
                | ')'
                | ']'
                | '}'
                | '>'
        )
    })
}

fn smart_context_parse_line_ranges(suffix: &str) -> Vec<SmartContextAppendixLineRange> {
    let Some(suffix) = suffix
        .strip_prefix('#')
        .or_else(|| suffix.strip_prefix(':'))
        .or_else(|| suffix.strip_prefix('?'))
    else {
        return Vec::new();
    };
    let suffix = suffix.strip_prefix("lines=").unwrap_or(suffix);
    suffix
        .split(',')
        .filter_map(smart_context_parse_line_range_segment)
        .collect()
}

fn smart_context_parse_line_range_segment(suffix: &str) -> Option<SmartContextAppendixLineRange> {
    let suffix = suffix
        .strip_prefix('L')
        .or_else(|| suffix.strip_prefix('l'))
        .unwrap_or(suffix);
    let (start, end) = suffix.split_once('-').unwrap_or((suffix, suffix));
    let start = smart_context_parse_line_number(start)?;
    let end = smart_context_parse_line_number(end)?;
    (start > 0 && end >= start).then_some(SmartContextAppendixLineRange { start, end })
}

fn smart_context_parse_line_number(value: &str) -> Option<usize> {
    value
        .strip_prefix('L')
        .or_else(|| value.strip_prefix('l'))
        .unwrap_or(value)
        .parse::<usize>()
        .ok()
}
