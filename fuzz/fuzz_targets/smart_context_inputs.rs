#![no_main]

use libfuzzer_sys::fuzz_target;
use runtime_proxy::{
    SmartContextArtifactRef, SmartContextExactAppendixRange, smart_context_artifact_line_range,
    smart_context_compact_line_refs_if_shorter, smart_context_extract_line_range,
    smart_context_hash_matches_text, smart_context_hash_text, smart_context_render_exact_appendix,
    smart_context_short_artifact_line_ref, smart_context_unsupported_json_shape_reason,
};

const MAX_INPUT_BYTES: usize = 64 * 1024;

fuzz_target!(|input: &[u8]| {
    if input.len() > MAX_INPUT_BYTES {
        return;
    }
    if let Ok(value) = serde_json::from_slice(input) {
        let _ = smart_context_unsupported_json_shape_reason(&value);
    }
    let Ok(text) = std::str::from_utf8(input) else {
        return;
    };

    let content_hash = smart_context_hash_text(text);
    assert!(smart_context_hash_matches_text(&content_hash, text));
    let artifact = SmartContextArtifactRef {
        id: content_hash.clone(),
        byte_len: text.len(),
        content_hash,
    };
    let start = usize::from(input.first().copied().unwrap_or_default()) % 128;
    let end = usize::from(input.get(1).copied().unwrap_or_default()) % 128;
    let extracted = smart_context_extract_line_range(text, start, end);
    let ranged = smart_context_artifact_line_range(&artifact, text, start, end);
    assert_eq!(
        ranged.as_ref().map(|range| &range.excerpt),
        extracted.as_ref()
    );

    let reference = smart_context_short_artifact_line_ref(&artifact.id, start, end);
    let _ = smart_context_render_exact_appendix(
        "Smart Context exact appendix",
        vec![SmartContextExactAppendixRange {
            reference: reference.clone(),
            body: text.to_string(),
        }],
    );
    let _ = smart_context_compact_line_refs_if_shorter(&[reference.clone(), reference]);
});
