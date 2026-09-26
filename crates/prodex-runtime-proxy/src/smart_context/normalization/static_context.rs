use super::*;

pub(in crate::smart_context) fn smart_context_static_context_prompt_cache_payload(
    items: &[SmartContextStableStaticContextItem],
) -> String {
    let mut payload = String::from("prodex-smart-context-static-prompt-cache-v1\n");
    for item in items {
        payload.push_str("id-bytes:");
        payload.push_str(&item.id.len().to_string());
        payload.push('\n');
        payload.push_str(&item.id);
        payload.push('\n');
        payload.push_str("text-bytes:");
        payload.push_str(&item.byte_len.to_string());
        payload.push('\n');
        payload.push_str(&item.canonical_text);
        payload.push('\n');
    }
    payload
}

pub(in crate::smart_context) fn smart_context_static_context_noise_line(line: &str) -> bool {
    prodex_mojo_core::rich::smart_context_static_context_noise_line(line)
        .expect("Mojo Smart Context static-context classifier returned invalid output")
}
