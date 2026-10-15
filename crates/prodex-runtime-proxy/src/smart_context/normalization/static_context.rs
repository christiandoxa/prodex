use super::*;

pub(in crate::smart_context) fn smart_context_static_context_prompt_cache_payload(
    items: &[SmartContextStableStaticContextItem],
) -> String {
    let sources = items
        .iter()
        .map(
            |item| prodex_mojo_core::rich::SmartContextStaticPayloadItem {
                id: &item.id,
                canonical_text: &item.canonical_text,
                byte_len: item.byte_len,
            },
        )
        .collect::<Vec<_>>();
    prodex_mojo_core::rich::smart_context_static_prompt_payload(&sources)
        .expect("Mojo Smart Context cache payload renderer returned invalid output")
}

#[cfg(test)]
pub(in crate::smart_context) fn smart_context_static_context_noise_line(line: &str) -> bool {
    prodex_mojo_core::rich::smart_context_static_context_noise_line(line)
        .expect("Mojo Smart Context static-context classifier returned invalid output")
}
