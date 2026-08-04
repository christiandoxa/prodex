use super::*;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::fmt::Write as _;

const SMART_CONTEXT_STATIC_CONTEXT_SECTION_MIN_BYTES: usize = 512;
pub const SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS: usize = 128;
pub const SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEM_BYTES: usize = 256 * 1024;
const SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ID_BYTES: usize = 256;
const SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX: &str = "psc static ";
const SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY: &str =
    "prodex static context unchanged ";
const SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX: &str = "psc static dup ";
const SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX: &str = "psc static chunk dup ";
const SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX: &str = "psc static section dup ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextArtifactLineRangeRef {
    pub artifact_id: String,
    pub artifact_content_hash: String,
    pub artifact_byte_len: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub excerpt_hash: String,
    pub excerpt_byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextArtifactLineRange {
    pub reference: SmartContextArtifactLineRangeRef,
    pub excerpt: String,
}

pub fn smart_context_artifact_line_range(
    artifact: &SmartContextArtifactRef,
    artifact_text: &str,
    start_line: usize,
    end_line: usize,
) -> Option<SmartContextArtifactLineRange> {
    if artifact.content_hash != smart_context_hash_text(artifact_text) {
        return None;
    }

    let excerpt = smart_context_extract_line_range(artifact_text, start_line, end_line)?;
    let reference = SmartContextArtifactLineRangeRef {
        artifact_id: artifact.id.clone(),
        artifact_content_hash: artifact.content_hash.clone(),
        artifact_byte_len: artifact.byte_len,
        start_line,
        end_line,
        excerpt_hash: smart_context_hash_text(&excerpt),
        excerpt_byte_len: excerpt.len(),
    };

    Some(SmartContextArtifactLineRange { reference, excerpt })
}

pub fn smart_context_extract_line_range(
    text: &str,
    start_line: usize,
    end_line: usize,
) -> Option<String> {
    if start_line == 0 || end_line < start_line {
        return None;
    }

    let mut selected = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line_number > end_line {
            break;
        }
        if line_number >= start_line {
            selected.push(line);
        }
    }

    (!selected.is_empty()).then(|| selected.join("\n"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextFingerprintKind {
    StaticContext,
    ConversationTurn,
    ToolOutput,
    Artifact,
    MemoryCapsule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextFingerprintInput {
    pub id: String,
    pub kind: SmartContextFingerprintKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextFingerprint {
    pub id: String,
    pub kind: SmartContextFingerprintKind,
    pub content_hash: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStaticContextItem {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStableStaticContextItem {
    pub id: String,
    pub canonical_text: String,
    pub content_hash: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStaticContextPromptCacheFingerprint {
    pub content_hash: String,
    pub items: Vec<SmartContextStaticContextItemFingerprint>,
    pub item_count: usize,
    pub byte_len: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStaticContextItemFingerprint {
    pub id_hash: String,
    pub content_hash: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStaticHeadingSection {
    pub heading: String,
    pub start: usize,
    pub end: usize,
    pub ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartContextFingerprintChange {
    Added {
        fingerprint: SmartContextFingerprint,
    },
    Removed {
        fingerprint: SmartContextFingerprint,
    },
    Unchanged {
        fingerprint: SmartContextFingerprint,
    },
    Changed {
        before: SmartContextFingerprint,
        after: SmartContextFingerprint,
    },
}

pub fn smart_context_fingerprint(input: SmartContextFingerprintInput) -> SmartContextFingerprint {
    SmartContextFingerprint {
        id: input.id,
        kind: input.kind,
        content_hash: smart_context_hash_text(&input.text),
        byte_len: input.text.len(),
    }
}

pub fn smart_context_fingerprints(
    inputs: impl IntoIterator<Item = SmartContextFingerprintInput>,
) -> Vec<SmartContextFingerprint> {
    inputs.into_iter().map(smart_context_fingerprint).collect()
}

pub fn smart_context_stabilize_static_context_text(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !smart_context_static_context_noise_line(line))
        .map(|line| smart_context_normalize_volatile_static_context(line).into_owned())
        .collect::<Vec<_>>();

    let Some(start) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return String::new();
    };
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .unwrap_or(start);

    lines[start..=end].join("\n")
}

pub fn smart_context_stabilize_static_context_items(
    items: impl IntoIterator<Item = SmartContextStaticContextItem>,
) -> Vec<SmartContextStableStaticContextItem> {
    let mut items = items
        .into_iter()
        .filter_map(|item| {
            let id = smart_context_stabilize_static_context_id(&item.id);
            let canonical_text = smart_context_stabilize_static_context_text(&item.text);
            if id.is_empty() && canonical_text.is_empty() {
                return None;
            }
            let content_hash = smart_context_hash_text(&canonical_text);
            Some(SmartContextStableStaticContextItem {
                id,
                byte_len: canonical_text.len(),
                canonical_text,
                content_hash,
            })
        })
        .collect::<Vec<_>>();

    items.sort_by(smart_context_static_context_item_order);
    items
}

pub fn smart_context_static_context_prompt_cache_fingerprint(
    items: impl IntoIterator<Item = SmartContextStaticContextItem>,
) -> SmartContextStaticContextPromptCacheFingerprint {
    let (items, item_count, byte_len, truncated, overflow_hash) =
        smart_context_stabilize_static_context_items_bounded(items);
    let mut payload = smart_context_static_context_prompt_cache_payload(&items);
    if truncated {
        payload.push_str("psc static fingerprint truncated ");
        payload.push_str(&item_count.to_string());
        payload.push(' ');
        payload.push_str(&byte_len.to_string());
        payload.push(' ');
        payload.push_str(&overflow_hash);
        payload.push('\n');
    }

    SmartContextStaticContextPromptCacheFingerprint {
        content_hash: smart_context_hash_text(&payload).replacen("sc2:", "scpc2:", 1),
        items: items
            .iter()
            .map(|item| SmartContextStaticContextItemFingerprint {
                id_hash: smart_context_hash_text(&item.id),
                content_hash: item.content_hash.clone(),
                byte_len: item.byte_len,
            })
            .collect(),
        item_count,
        byte_len,
        truncated,
    }
}

fn smart_context_stabilize_static_context_items_bounded(
    items: impl IntoIterator<Item = SmartContextStaticContextItem>,
) -> (
    Vec<SmartContextStableStaticContextItem>,
    usize,
    usize,
    bool,
    String,
) {
    let mut stable = Vec::new();
    let mut item_count = 0usize;
    let mut byte_len = 0usize;
    let mut truncated = false;
    let mut overflow_digest = [0u8; 32];

    for item in items {
        let id = smart_context_bounded_static_context_id(&item.id);
        let canonical_text = smart_context_bounded_static_context_text(&item.text);
        if id.is_empty() && canonical_text.is_empty() {
            continue;
        }
        item_count = item_count.saturating_add(1);
        byte_len = byte_len.saturating_add(canonical_text.len());
        let candidate = SmartContextStableStaticContextItem {
            id,
            byte_len: canonical_text.len(),
            content_hash: smart_context_hash_text(&canonical_text),
            canonical_text,
        };
        if stable.len() < SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS {
            stable.push(candidate);
            continue;
        }

        truncated = true;
        let largest_index = stable
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| smart_context_static_context_item_order(left, right))
            .map(|(index, _)| index)
            .expect("a full bounded fingerprint set is non-empty");
        if smart_context_static_context_item_order(&candidate, &stable[largest_index]).is_lt() {
            let overflow = std::mem::replace(&mut stable[largest_index], candidate);
            smart_context_add_static_context_overflow_digest(&mut overflow_digest, &overflow);
        } else {
            smart_context_add_static_context_overflow_digest(&mut overflow_digest, &candidate);
        }
    }

    stable.sort_by(smart_context_static_context_item_order);
    (
        stable,
        item_count,
        byte_len,
        truncated,
        smart_context_hex_digest(overflow_digest),
    )
}

fn smart_context_add_static_context_overflow_digest(
    aggregate: &mut [u8; 32],
    item: &SmartContextStableStaticContextItem,
) {
    let mut item_hasher = Sha256::new();
    item_hasher.update(item.id.as_bytes());
    item_hasher.update([0]);
    item_hasher.update(item.content_hash.as_bytes());
    item_hasher.update([0]);
    item_hasher.update(item.byte_len.to_le_bytes());
    for (slot, byte) in aggregate
        .iter_mut()
        .zip(item_hasher.finalize().iter().copied())
    {
        *slot = slot.wrapping_add(byte);
    }
}

fn smart_context_bounded_static_context_id(id: &str) -> String {
    let trimmed = id.trim();
    if trimmed.len() <= SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ID_BYTES {
        return trimmed.replace('\\', "/");
    }
    let mut end = SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ID_BYTES;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}<id-hash={}>",
        trimmed[..end].replace('\\', "/"),
        smart_context_hash_text(id)
    )
}

fn smart_context_bounded_static_context_text(text: &str) -> String {
    if text.len() <= SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEM_BYTES {
        return smart_context_stabilize_static_context_text(text);
    }
    let mut end = SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEM_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let prefix = smart_context_stabilize_static_context_text(&text[..end]);
    format!(
        "{prefix}\npsc static fingerprint item truncated bytes={} hash={}",
        text.len(),
        smart_context_hash_text(text)
    )
}

fn smart_context_hex_digest(digest: [u8; 32]) -> String {
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub fn smart_context_static_heading_section_body<'a>(
    text: &'a str,
    section: &SmartContextStaticHeadingSection,
) -> Option<&'a str> {
    if section.start >= section.end
        || section.end > text.len()
        || !text.is_char_boundary(section.start)
        || !text.is_char_boundary(section.end)
    {
        return None;
    }
    text.get(section.start..section.end)
}

pub fn smart_context_static_context_heading_sections(
    text: &str,
) -> Vec<SmartContextStaticHeadingSection> {
    let mut headings = Vec::<(String, usize)>::new();
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches('\n').trim_end_matches('\r');
        if let Some(heading) = smart_context_static_context_heading(line_without_newline) {
            headings.push((heading, offset));
        }
        offset = offset.saturating_add(line.len());
    }
    if !text.ends_with('\n')
        && let Some(last_line) = text.rsplit('\n').next()
        && let Some(heading) = smart_context_static_context_heading(last_line)
    {
        let start = text.len().saturating_sub(last_line.len());
        if !headings
            .iter()
            .any(|(_, existing_start)| *existing_start == start)
        {
            headings.push((heading, start));
        }
    }
    let mut sections = Vec::new();
    for (index, (heading, start)) in headings.iter().enumerate() {
        let end = headings
            .get(index + 1)
            .map(|(_, next_start)| *next_start)
            .unwrap_or(text.len());
        if end.saturating_sub(*start) < SMART_CONTEXT_STATIC_CONTEXT_SECTION_MIN_BYTES {
            continue;
        }
        let Some(body) = text.get(*start..end).map(str::trim) else {
            continue;
        };
        if body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DELTA_MARKER_PREFIX_LEGACY)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_DUP_MARKER_PREFIX)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_CHUNK_DUP_MARKER_PREFIX)
            || body.starts_with(SMART_CONTEXT_STATIC_CONTEXT_SECTION_DUP_MARKER_PREFIX)
        {
            continue;
        }
        sections.push(SmartContextStaticHeadingSection {
            heading: heading.clone(),
            start: *start,
            end,
            ordinal: index,
        });
    }
    sections
}

fn smart_context_static_context_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if level == 0 || level > 6 || !trimmed.chars().nth(level).is_some_and(char::is_whitespace) {
        return None;
    }
    Some(trimmed.to_string())
}

pub fn smart_context_fingerprint_delta(
    previous: impl IntoIterator<Item = SmartContextFingerprint>,
    current: impl IntoIterator<Item = SmartContextFingerprint>,
) -> Vec<SmartContextFingerprintChange> {
    let previous = smart_context_fingerprint_map(previous);
    let current = smart_context_fingerprint_map(current);
    let mut keys = BTreeSet::new();
    keys.extend(previous.keys().cloned());
    keys.extend(current.keys().cloned());

    keys.into_iter()
        .filter_map(|key| match (previous.get(&key), current.get(&key)) {
            (None, Some(after)) => Some(SmartContextFingerprintChange::Added {
                fingerprint: after.clone(),
            }),
            (Some(before), None) => Some(SmartContextFingerprintChange::Removed {
                fingerprint: before.clone(),
            }),
            (Some(before), Some(after)) if before.content_hash == after.content_hash => {
                Some(SmartContextFingerprintChange::Unchanged {
                    fingerprint: after.clone(),
                })
            }
            (Some(before), Some(after)) => Some(SmartContextFingerprintChange::Changed {
                before: before.clone(),
                after: after.clone(),
            }),
            (None, None) => None,
        })
        .collect()
}
