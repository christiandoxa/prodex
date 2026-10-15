use super::{gemini_provider_core_coalesce_contents, gemini_provider_core_harden_contents};
use serde_json::json;

#[test]
fn mojo_coalescing_preserves_first_metadata_and_order_across_invalid_contents() {
    let mut contents = vec![
        json!(null),
        json!({"role": "user", "parts": [{"text": "Tokyo"}], "first": {"keep": true}}),
        json!({"role": "model", "parts": []}),
        json!({"role": "model", "parts": "invalid"}),
        json!({"parts": [null, {"text": "second"}], "later_metadata": 7}),
        json!({"role": "model", "parts": [{"text": "reply"}], "keep_model": [1, 2]}),
        json!({"role": "model", "parts": [false, 3], "later_model": true}),
        json!({"role": 42, "parts": [{"text": "next"}]}),
    ];
    gemini_provider_core_coalesce_contents(&mut contents);
    assert_eq!(
        contents,
        vec![
            json!({"role": "user", "parts": [{"text": "Tokyo"}, null, {"text": "second"}], "first": {"keep": true}}),
            json!({"role": "model", "parts": [{"text": "reply"}, false, 3], "keep_model": [1, 2]}),
            json!({"role": "user", "parts": [{"text": "next"}]}),
        ]
    );
    let once = contents.clone();
    gemini_provider_core_coalesce_contents(&mut contents);
    assert_eq!(contents, once);
}

#[test]
fn mojo_coalescing_distinguishes_empty_custom_and_model_roles() {
    let mut contents = vec![
        json!({"role": "", "parts": [1]}),
        json!({"role": "", "parts": [2]}),
        json!({"role": "モデル", "parts": [3]}),
        json!({"role": "model", "parts": [4]}),
    ];
    gemini_provider_core_coalesce_contents(&mut contents);
    assert_eq!(
        contents,
        vec![
            json!({"role": "", "parts": [1, 2]}),
            json!({"role": "モデル", "parts": [3]}),
            json!({"role": "model", "parts": [4]}),
        ]
    );
    let mut empty = vec![json!(false), json!({"parts": []}), json!({"parts": null})];
    gemini_provider_core_coalesce_contents(&mut empty);
    assert!(empty.is_empty());
}

#[test]
fn mojo_coalescing_is_used_by_complete_gemini_hardening() {
    let mut contents = vec![json!({"role": "model", "parts": [{"text": "answer"}]})];
    gemini_provider_core_harden_contents(&mut contents);
    assert_eq!(
        contents,
        vec![
            json!({"role": "user", "parts": [{"text": "[Continuing from previous Codex context.]"}]}),
            json!({"role": "model", "parts": [{"text": "answer"}]}),
            json!({"role": "user", "parts": [{"text": "Please continue."}]}),
        ]
    );
}

#[test]
fn mojo_role_shape_coalesces_synthetic_boundaries_using_original_metadata_rules() {
    let mut contents = vec![
        json!({"role": "model", "parts": []}),
        json!({"role": "user", "parts": [{"text": "existing"}], "metadata": "must not replace synthetic first"}),
        json!({"role": "model", "parts": false}),
    ];
    super::gemini_provider_core_enforce_content_role_shape(&mut contents);
    assert_eq!(
        contents,
        vec![json!({"role": "user", "parts": [
            {"text": "[Continuing from previous Codex context.]"},
            {"text": "existing"},
            {"text": "Please continue."},
        ]})]
    );
}

#[test]
fn mojo_role_shape_preserves_the_raw_boundary_decision_before_invalid_rows_are_removed() {
    let mut contents = vec![
        json!(null),
        json!({"role": "model", "parts": [{"text": "reply"}]}),
    ];
    super::gemini_provider_core_enforce_content_role_shape(&mut contents);
    assert_eq!(
        contents,
        vec![
            json!({"role": "model", "parts": [{"text": "reply"}]}),
            json!({"role": "user", "parts": [{"text": "Please continue."}]}),
        ]
    );
    let mut all_invalid = vec![json!({"role": "model", "parts": false})];
    super::gemini_provider_core_enforce_content_role_shape(&mut all_invalid);
    assert_eq!(
        all_invalid,
        vec![json!({"role":"user", "parts": [
            {"text": "[Continuing from previous Codex context.]"}, {"text": "Please continue."},
        ]})]
    );
}
