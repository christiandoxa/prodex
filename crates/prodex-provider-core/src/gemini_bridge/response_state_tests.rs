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

#[cfg(feature = "mojo")]
#[test]
fn mojo_response_part_plan_matches_rust_oracle() {
    let mut cases = Vec::new();
    for has_text in [false, true] {
        for is_thought in [false, true] {
            for has_visible_text in [false, true] {
                for has_special_text in [false, true] {
                    for has_media in [false, true] {
                        for has_video_metadata in [false, true] {
                            for has_image_generation in [false, true] {
                                for has_function_call in [false, true] {
                                    cases.push(GeminiProviderCoreResponsePartInput {
                                        has_text,
                                        is_thought,
                                        has_visible_text,
                                        has_special_text,
                                        has_media,
                                        has_video_metadata,
                                        has_image_generation,
                                        has_function_call,
                                        command_output_only: false,
                                        forced_output: false,
                                        internal_instruction_echo: false,
                                        suppress_visible_text: false,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for input in cases {
        let expected = gemini_provider_core_response_part_plan_rust(input);
        let actual = gemini_provider_core_response_part_plan(input).unwrap();
        let actual = [
            (actual.emit_reasoning, ACTION_REASONING),
            (actual.emit_visible_text, ACTION_VISIBLE_TEXT),
            (actual.emit_special_text, ACTION_SPECIAL_TEXT),
            (actual.record_media, ACTION_MEDIA),
            (actual.record_native, ACTION_NATIVE),
            (actual.record_image, ACTION_IMAGE),
            (actual.emit_function, ACTION_FUNCTION),
            (actual.flush_pending, ACTION_FLUSH_PENDING),
        ]
        .into_iter()
        .filter_map(|(enabled, action)| enabled.then_some(action))
        .fold(0, |actions, action| actions | action);
        assert_eq!(actual, expected, "input={input:?}");
    }
}
