//! Gemini exact command-output policy. Conversation ordering, request intent,
//! required command discovery and tool text projection are compiled in Mojo.

mod commands;

use self::commands::{
    gemini_provider_core_text_contains_required_exact_output_marker,
    gemini_provider_core_tool_command_matches_required,
};
use super::tool_text::gemini_provider_core_guardrail_projection;
use prodex_mojo_core::gemini_guardrails::GeminiGuardrailJsonOperation;

// Typed JSON plan: requested, latest user, latest subsequent tool,
// optional exact command and optional extracted tool output.
type GeminiExactOutputPlan = (
    bool,
    Option<usize>,
    Option<usize>,
    Option<String>,
    Option<String>,
);

fn gemini_exact_output_plan(messages: &[serde_json::Value]) -> GeminiExactOutputPlan {
    let plan: GeminiExactOutputPlan = gemini_provider_core_guardrail_projection(
        &serde_json::json!({"messages": messages}),
        GeminiGuardrailJsonOperation::ExactOutputSelection,
    );
    // Fail closed on an invalid compiled contract rather than trying to
    // reselect messages using an alternative Rust policy.
    if plan.0 {
        assert!(
            plan.1.is_some_and(|index| index < messages.len()),
            "compiled Mojo selected an invalid exact-output user"
        );
    } else {
        assert!(plan.2.is_none() && plan.3.is_none() && plan.4.is_none());
    }
    if let Some(tool) = plan.2 {
        assert!(
            plan.1.is_some_and(|user| user < tool) && tool < messages.len(),
            "compiled Mojo selected an invalid exact-output tool"
        );
    } else {
        assert!(plan.4.is_none());
    }
    plan
}

pub fn gemini_provider_core_forced_command_output(
    messages: &[serde_json::Value],
) -> Option<String> {
    let (requested, user, tool, required_command, output) = gemini_exact_output_plan(messages);
    if !requested {
        return None;
    }
    let user = user.expect("compiled Mojo output-only request must have a user");
    let tool = tool?;
    let tool_output = output?;
    if let Some(required_command) = required_command
        && !gemini_provider_core_tool_command_matches_required(
            &messages[user + 1..tool],
            &required_command,
        )
        && !gemini_provider_core_text_contains_required_exact_output_marker(
            &required_command,
            &tool_output,
        )
    {
        return None;
    }
    Some(tool_output)
}

pub fn gemini_provider_core_conversation_requests_command_output_only(
    messages: &[serde_json::Value],
) -> bool {
    gemini_exact_output_plan(messages).0
}
