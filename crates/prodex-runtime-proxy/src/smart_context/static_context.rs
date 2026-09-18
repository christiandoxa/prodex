use super::*;
use sha2::{Digest as _, Sha256};
#[cfg(feature = "mojo")]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write as _;

pub const SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS: usize = 128;
pub const SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEM_BYTES: usize = 256 * 1024;
const SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ID_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextFingerprintKind {
    StaticContext,
    ConversationTurn,
    ToolOutput,
    Artifact,
    MemoryCapsule,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SmartContextFingerprintInput {
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

#[cfg(test)]
pub(crate) fn smart_context_fingerprint(
    input: SmartContextFingerprintInput,
) -> SmartContextFingerprint {
    SmartContextFingerprint {
        id: input.id,
        kind: input.kind,
        content_hash: smart_context_hash_text(&input.text),
        byte_len: input.text.len(),
    }
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

        #[cfg(feature = "mojo")]
        {
            stable.push(candidate);
            if stable.len() == SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS + 16 {
                stable = smart_context_reduce_static_context_items_mojo(
                    stable,
                    &mut overflow_digest,
                    SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS,
                );
                truncated = true;
            }
            continue;
        }

        #[cfg(not(feature = "mojo"))]
        if stable.len() < SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS {
            stable.push(candidate);
            continue;
        }

        #[cfg(not(feature = "mojo"))]
        {
            truncated = true;
            let largest_index = stable
                .iter()
                .enumerate()
                .max_by(|(_, left), (_, right)| {
                    smart_context_static_context_item_order(left, right)
                })
                .map(|(index, _)| index)
                .expect("a full bounded fingerprint set is non-empty");
            if smart_context_static_context_item_order(&candidate, &stable[largest_index]).is_lt() {
                let overflow = std::mem::replace(&mut stable[largest_index], candidate);
                smart_context_add_static_context_overflow_digest(&mut overflow_digest, &overflow);
            } else {
                smart_context_add_static_context_overflow_digest(&mut overflow_digest, &candidate);
            }
        }
    }

    #[cfg(feature = "mojo")]
    {
        let maximum_items = stable
            .len()
            .min(SMART_CONTEXT_STATIC_CONTEXT_FINGERPRINT_MAX_ITEMS);
        truncated |= stable.len() > maximum_items;
        stable = smart_context_reduce_static_context_items_mojo(
            stable,
            &mut overflow_digest,
            maximum_items,
        );
    }

    #[cfg(not(feature = "mojo"))]
    stable.sort_by(smart_context_static_context_item_order);
    (
        stable,
        item_count,
        byte_len,
        truncated,
        smart_context_hex_digest(overflow_digest),
    )
}

#[cfg(feature = "mojo")]
fn smart_context_reduce_static_context_items_mojo(
    items: Vec<SmartContextStableStaticContextItem>,
    overflow_digest: &mut [u8; 32],
    maximum_items: usize,
) -> Vec<SmartContextStableStaticContextItem> {
    let inputs = items
        .iter()
        .map(
            |item| prodex_mojo_core::runtime::SmartContextStaticItemPlanInput {
                id: &item.id,
                content_hash: &item.content_hash,
                canonical_text: &item.canonical_text,
                byte_len: item.byte_len as u64,
            },
        )
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::runtime::smart_context_static_item_plan(&inputs, maximum_items)
        .expect("Mojo Smart Context static item plan returned invalid output");
    let mut items = items.into_iter().map(Some).collect::<Vec<_>>();
    let selected = plan
        .selected_indices
        .into_iter()
        .map(|index| items[index].take().expect("Mojo indices are unique"))
        .collect::<Vec<_>>();
    for overflow in items.into_iter().flatten() {
        smart_context_add_static_context_overflow_digest(overflow_digest, &overflow);
    }
    selected
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

pub fn smart_context_fingerprint_delta(
    previous: impl IntoIterator<Item = SmartContextFingerprint>,
    current: impl IntoIterator<Item = SmartContextFingerprint>,
) -> Vec<SmartContextFingerprintChange> {
    #[cfg(feature = "mojo")]
    {
        let previous = previous.into_iter().collect::<Vec<_>>();
        let current = current.into_iter().collect::<Vec<_>>();
        smart_context_fingerprint_delta_mojo(previous, current)
            .expect("Mojo Smart Context fingerprint delta plan returned invalid output")
    }

    #[cfg(not(feature = "mojo"))]
    smart_context_fingerprint_delta_rust(previous, current)
}

#[cfg(feature = "mojo")]
fn smart_context_fingerprint_delta_mojo(
    previous: Vec<SmartContextFingerprint>,
    current: Vec<SmartContextFingerprint>,
) -> Result<Vec<SmartContextFingerprintChange>, prodex_mojo_core::MojoError> {
    let keys = previous
        .iter()
        .chain(&current)
        .map(|fingerprint| (fingerprint.kind, fingerprint.id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(|(rank, key)| (key, rank as u64))
        .collect::<BTreeMap<_, _>>();
    let hashes = previous
        .iter()
        .chain(&current)
        .map(|fingerprint| fingerprint.content_hash.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(|(rank, hash)| (hash, rank as u64))
        .collect::<BTreeMap<_, _>>();
    let encode = |fingerprints: &[SmartContextFingerprint]| {
        fingerprints
            .iter()
            .map(
                |fingerprint| prodex_mojo_core::runtime::SmartContextFingerprintDeltaInput {
                    key: keys[&(fingerprint.kind, fingerprint.id.clone())],
                    content_hash: hashes[fingerprint.content_hash.as_str()],
                },
            )
            .collect::<Vec<_>>()
    };
    prodex_mojo_core::runtime::smart_context_fingerprint_delta_plan(
        &encode(&previous),
        &encode(&current),
        keys.len(),
    )?
    .into_iter()
    .map(
        |item| match (item.action, item.previous_index, item.current_index) {
            (0, None, Some(after)) => Ok(SmartContextFingerprintChange::Added {
                fingerprint: current[after].clone(),
            }),
            (1, Some(before), None) => Ok(SmartContextFingerprintChange::Removed {
                fingerprint: previous[before].clone(),
            }),
            (2, Some(_), Some(after)) => Ok(SmartContextFingerprintChange::Unchanged {
                fingerprint: current[after].clone(),
            }),
            (3, Some(before), Some(after)) => Ok(SmartContextFingerprintChange::Changed {
                before: previous[before].clone(),
                after: current[after].clone(),
            }),
            _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
        },
    )
    .collect()
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_fingerprint_delta_rust(
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

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    fn fingerprint(
        id: &str,
        kind: SmartContextFingerprintKind,
        hash: &str,
    ) -> SmartContextFingerprint {
        SmartContextFingerprint {
            id: id.to_string(),
            kind,
            content_hash: hash.to_string(),
            byte_len: hash.len(),
        }
    }

    #[test]
    fn fingerprint_delta_matches_rust_oracle_with_duplicate_keys() {
        let previous = vec![
            fingerprint("b", SmartContextFingerprintKind::StaticContext, "old"),
            fingerprint("a", SmartContextFingerprintKind::Artifact, "same"),
            fingerprint("b", SmartContextFingerprintKind::StaticContext, "before"),
            fingerprint("removed", SmartContextFingerprintKind::ToolOutput, "gone"),
        ];
        let current = vec![
            fingerprint(
                "added",
                SmartContextFingerprintKind::ConversationTurn,
                "new",
            ),
            fingerprint("b", SmartContextFingerprintKind::StaticContext, "after"),
            fingerprint("a", SmartContextFingerprintKind::Artifact, "same"),
        ];
        assert_eq!(
            smart_context_fingerprint_delta(previous.clone(), current.clone()),
            smart_context_fingerprint_delta_rust(previous, current)
        );
    }
}
