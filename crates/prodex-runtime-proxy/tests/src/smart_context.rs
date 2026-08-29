use super::*;
use std::borrow::Cow;

#[path = "smart_context/golden.rs"]
mod golden;

#[path = "smart_context/core_artifacts.rs"]
mod core_artifacts;

#[path = "smart_context/candidates.rs"]
mod candidates;

#[path = "smart_context/path_aliases.rs"]
mod path_aliases;

#[path = "smart_context/memory_budget.rs"]
mod memory_budget;

#[path = "smart_context/model_registry.rs"]
mod model_registry;

#[path = "smart_context/rehydration.rs"]
mod rehydration;

#[path = "smart_context/rollout.rs"]
mod rollout;

#[path = "smart_context/replay.rs"]
mod replay;

#[path = "smart_context/token_accounting.rs"]
mod token_accounting;

#[path = "smart_context/static_context.rs"]
mod static_context;

#[path = "smart_context/adaptive_rewrite.rs"]
mod adaptive_rewrite;

#[path = "smart_context/safety.rs"]
mod safety;

#[path = "smart_context/tool_outputs.rs"]
mod tool_outputs;

#[cfg(feature = "mojo")]
#[test]
fn token_accounting_matches_rust_oracle_for_generated_inputs() {
    let mut state = 0x746f6b656e5f6163_u64;
    for case in 0..300 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let observed_usage = (0..(state % 8) as usize)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                RuntimeTokenUsage {
                    input_tokens: state % 200_000,
                    cached_input_tokens: state.rotate_left(17) % 50_000,
                    output_tokens: state.rotate_right(11) % 20_000,
                    reasoning_tokens: state.rotate_left(29) % 10_000,
                }
            })
            .collect::<Vec<_>>();
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let input = SmartContextObservedTokenAccountingCalibrationInput {
            accounting: SmartContextObservedTokenAccountingInput {
                model_context_window_tokens: (state & 1 == 0).then_some(state % 300_000),
                reserved_output_tokens: state.rotate_left(7) % 60_000,
                current_input_tokens: state.rotate_right(13) % 200_000,
                current_request_body_bytes: (state.rotate_left(23) % 300_000) as usize,
                current_request_estimated_tokens: (state & 2 != 0)
                    .then(|| state.rotate_right(31) % 100_000),
                observed_usage,
            },
            calibration_bucket_key: None,
            calibration_samples: Vec::new(),
        };
        let expected =
            super::token_accounting::oracle::smart_context_observed_token_accounting_rust(
                input.clone(),
            );
        let actual = smart_context_observed_token_accounting_with_calibration(input);
        assert_eq!(actual, expected, "token accounting case {case}");
    }
}
