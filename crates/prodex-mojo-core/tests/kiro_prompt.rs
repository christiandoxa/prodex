#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::rich::{KiroKernelInput, KiroKernelOperation, kiro_kernel};

fn prompt(messages: &str) -> String {
    let mut input = KiroKernelInput::new(KiroKernelOperation::PromptFromChatMessages);
    input.input = Some(messages);
    String::from_utf8(kiro_kernel(input).expect("real Kiro prompt kernel succeeds"))
        .expect("Kiro prompt is UTF-8")
}

#[test]
fn kiro_prompt_recursively_applies_text_content_output_precedence_and_unicode_trim() {
    assert_eq!(
        prompt(
            r#"[
                {"role":"system","content":[
                    "\u2003 ",
                    {"text":"  plan  ","content":"ignored","output":"ignored"},
                    {"text":7,"content":[{"output":"nested"}]},
                    {"content":"\t","output":"from output"},
                    {"text":" ","content":"text wins even when blank","output":"ignored too"}
                ]},
                {"role":"assistant","content":"\u00a0 model \t","tool_calls":[
                    {"function":{"name":"run","arguments":"{\"cmd\":\"x\"}"}},
                    {"function":{"name":"","arguments":3}},
                    false
                ]},
                {"role":"tool","content":{"output":"\u2003tool result\t"}},
                {"role":"custom","content":" \u2003"},
                {"content":[{"content":"last"}]}
            ]"#
        ),
        "System:\nplan  \nnested\nfrom output\n\nAssistant:\nmodel \t\nTool call run: {\"cmd\":\"x\"}\nTool call : {}\nTool call tool_call: {}\n\nTool:\ntool result\n\nUser:\nlast"
    );
}

#[test]
fn kiro_prompt_joins_only_nonempty_sections_and_uses_empty_fallback() {
    assert_eq!(
        prompt(r#"[{},null,{"role":"assistant","content":[{"text":"\u3000"}]}]"#),
        "User:\n"
    );
    assert_eq!(prompt("[]"), "User:\n");
}
