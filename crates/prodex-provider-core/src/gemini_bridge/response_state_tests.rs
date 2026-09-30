use super::*;

fn input() -> GeminiProviderCoreResponsePartInput {
    GeminiProviderCoreResponsePartInput {
        has_text: false,
        is_thought: false,
        has_visible_text: false,
        has_special_text: false,
        has_media: false,
        has_video_metadata: false,
        has_image_generation: false,
        has_function_call: false,
        command_output_only: false,
        forced_output: false,
        internal_instruction_echo: false,
        suppress_visible_text: false,
    }
}

#[test]
fn response_part_plan_preserves_stream_and_buffered_actions() {
    let mut text = input();
    text.has_text = true;
    text.has_visible_text = true;
    assert_eq!(
        gemini_provider_core_response_part_plan(text),
        Ok(GeminiProviderCoreResponsePartPlan {
            emit_reasoning: false,
            emit_visible_text: true,
            emit_special_text: false,
            record_media: false,
            record_native: false,
            record_image: false,
            emit_function: false,
            flush_pending: false,
        })
    );

    let mut thought = input();
    thought.has_text = true;
    thought.is_thought = true;
    thought.has_visible_text = true;
    assert!(
        gemini_provider_core_response_part_plan(thought)
            .is_ok_and(|plan| plan.emit_reasoning && !plan.emit_visible_text)
    );

    let mut function = input();
    function.has_function_call = true;
    function.has_visible_text = true;
    function.suppress_visible_text = true;
    assert!(
        gemini_provider_core_response_part_plan(function)
            .is_ok_and(|plan| plan.emit_function && plan.flush_pending && !plan.emit_visible_text)
    );

    let mut media = input();
    media.has_media = true;
    media.has_video_metadata = true;
    media.has_image_generation = true;
    assert!(
        gemini_provider_core_response_part_plan(media)
            .is_ok_and(|plan| { plan.record_media && plan.record_native && plan.record_image })
    );
}

#[test]
fn lifecycle_and_completion_guardrail_precedence_is_mojo_authoritative() {
    assert!(gemini_provider_core_once_event_should_emit(
        false, true, true
    ));
    assert!(!gemini_provider_core_once_event_should_emit(
        true, true, true
    ));
    assert!(!gemini_provider_core_once_event_should_emit(
        false, false, true
    ));
    assert!(gemini_provider_core_once_event_should_emit(
        false, false, false
    ));

    assert_eq!(
        gemini_provider_core_completion_guardrail_action(true, true, true, true, true, true),
        GeminiProviderCoreCompletionGuardrailAction::EmptyResponse
    );
    assert_eq!(
        gemini_provider_core_completion_guardrail_action(false, true, true, true, true, true),
        GeminiProviderCoreCompletionGuardrailAction::ToolIntentWithoutCall
    );
    assert_eq!(
        gemini_provider_core_completion_guardrail_action(false, true, true, false, true, true),
        GeminiProviderCoreCompletionGuardrailAction::NonActionableWait
    );
    assert_eq!(
        gemini_provider_core_completion_guardrail_action(false, true, true, false, false, true),
        GeminiProviderCoreCompletionGuardrailAction::UnverifiedSuccess
    );
    assert_eq!(
        gemini_provider_core_completion_guardrail_action(false, true, false, true, true, true),
        GeminiProviderCoreCompletionGuardrailAction::None
    );
}
