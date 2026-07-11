use super::*;
use std::borrow::Cow;

#[test]
fn smart_context_generated_summary_uses_aliases_only_when_shorter() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let refs = (1usize..=10)
        .map(|line| runtime_smart_context_artifact_line_ref(&artifact.id, line.min(4), line.min(4)))
        .collect::<Vec<_>>()
        .join("\n");
    let marker = runtime_smart_context_artifact_marker_line("artifact", &artifact);
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!("{marker}\n{SMART_CONTEXT_LABEL_CRITICAL_EXACT}\n{refs}")
        }]
    });
    let before = value["input"][0]["output"].as_str().unwrap().len();

    assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts(&mut value));

    let output = value["input"][0]["output"].as_str().unwrap();
    assert!(output.contains("psc a @0=psc:"));
    assert!(output.contains("@0#L1-L1"));
    assert!(output.len() < before);
}

#[test]
fn smart_context_generated_summary_uses_path_aliases_only_when_shorter() {
    let repo = "/workspace/prodex";
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!(
                "psc manifest 3 artifacts\n{repo}/crates/prodex-app/src/runtime_proxy/smart_context.rs\n{repo}/crates/prodex-app/tests/src/runtime_proxy/smart_context.rs\n{repo}/crates/prodex-runtime-proxy/src/smart_context.rs"
            )
        }]
    });
    let before = value["input"][0]["output"].as_str().unwrap().len();

    assert!(runtime_smart_context_apply_path_aliases_to_generated_texts(
        &mut value
    ));

    let output = value["input"][0]["output"].as_str().unwrap();
    assert!(output.contains("psc p $R=/workspace/prodex"));
    assert!(output.contains("$R/crates/prodex-app/src/runtime_proxy/smart_context.rs"));
    assert!(output.len() < before);
}

#[test]
fn smart_context_prepare_aliases_existing_generated_paths_without_new_transform() {
    let shared = smart_context_test_shared("existing-generated-path-aliases");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let repo = "/workspace/prodex";
    let request = smart_context_test_request(serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!(
                "psc m refs-only\n{repo}/crates/prodex-app/src/runtime_proxy/smart_context.rs\n{repo}/crates/prodex-app/tests/src/runtime_proxy/smart_context.rs\n{repo}/crates/prodex-runtime-proxy/src/smart_context.rs"
            )
        }]
    }));
    let before_len = request.body.len();

    let prepared = prepare_runtime_smart_context_http_body(
        135,
        &request,
        &shared,
        RuntimeRouteKind::Responses,
    );

    let Cow::Owned(body) = prepared else {
        panic!("expected generated paths to be aliased");
    };
    let output = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["input"][0]["output"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(body.len() < before_len);
    assert!(output.contains("psc p $R=/workspace/prodex"));
    assert!(output.contains("$R/crates/prodex-app/src/runtime_proxy/smart_context.rs"));
    assert!(!output.contains("/workspace/prodex/crates/prodex-app/src/runtime_proxy"));
}

#[test]
fn smart_context_generated_summary_dedupes_existing_alias_legend() {
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let ref_line = runtime_smart_context_artifact_line_ref(&artifact.id, 1, 1);
    let mut value = serde_json::json!({
        "input": [{
            "type": "function_call_output",
            "call_id": "call_1",
            "output": format!(
                "psc art {}\npsc a @0={}\n{ref_line}\n{ref_line}\n{ref_line}",
                runtime_smart_context_artifact_ref(&artifact.id),
                runtime_smart_context_artifact_ref(&artifact.id),
            )
        }]
    });

    assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts(&mut value));

    let output = value["input"][0]["output"].as_str().unwrap();
    assert_eq!(output.matches("psc a ").count(), 1);
    assert!(output.contains("@0#L1-L1"));
}

#[test]
fn smart_context_generated_summary_keeps_stateful_artifact_alias_stable() {
    let shared = smart_context_test_shared("stable-artifact-alias");
    register_runtime_smart_context_proxy_state(&shared.log_path, true, None, None);
    let mut store = RuntimeSmartContextArtifactStore::default();
    let first_artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let second_artifact = store.insert_text(2, "alpha\nbeta\ngamma\ndelta").unwrap();
    let first_refs = (1usize..=8)
        .map(|line| {
            runtime_smart_context_artifact_line_ref(&first_artifact.id, line.min(4), line.min(4))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let second_refs = (1usize..=8)
        .map(|line| {
            runtime_smart_context_artifact_line_ref(&second_artifact.id, line.min(4), line.min(4))
        })
        .collect::<Vec<_>>()
        .join("\n");

    with_runtime_smart_context_proxy_state(&shared, |state| {
        let mut first = serde_json::json!({
            "input": [{"type": "function_call_output", "output": format!("psc m refs-only\n{first_refs}")}]
        });
        assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
            &mut first,
            state,
        ));
        assert!(
            first["input"][0]["output"]
                .as_str()
                .unwrap()
                .contains("psc a @0=psc:")
        );

        let mut second = serde_json::json!({
            "input": [{"type": "function_call_output", "output": format!("psc m refs-only\n{second_refs}\n{first_refs}")}]
        });
        assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
            &mut second,
            state,
        ));
        let output = second["input"][0]["output"].as_str().unwrap();
        assert!(output.contains(&format!(
            "@0={}",
            runtime_smart_context_artifact_ref(&first_artifact.id)
        )));
        assert!(output.contains("@0#L1-L1"));
    })
    .unwrap();
}

#[test]
fn smart_context_persists_stateful_artifact_aliases_across_start() {
    let first_shared = smart_context_test_shared("stable-artifact-alias-persist-first");
    let artifact_path = first_shared
        .log_path
        .with_file_name("stable-artifact-alias-persist-artifacts.json");
    let calibration_path = runtime_smart_context_token_calibration_path(&artifact_path);
    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    register_runtime_smart_context_proxy_state(
        &first_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    let mut store = RuntimeSmartContextArtifactStore::default();
    let artifact = store
        .insert_text(1, "line one\nline two\nline three\nline four")
        .unwrap();
    let refs = (1usize..=8)
        .map(|line| runtime_smart_context_artifact_line_ref(&artifact.id, line.min(4), line.min(4)))
        .collect::<Vec<_>>()
        .join("\n");
    with_runtime_smart_context_proxy_state(&first_shared, |state| {
        let mut value = serde_json::json!({
            "input": [{"type": "function_call_output", "output": format!("psc m refs-only\n{refs}")}]
        });
        assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
            &mut value,
            state,
        ));
    })
    .unwrap();
    persist_runtime_smart_context_token_calibration_metadata(
        &first_shared,
        "smart_context_artifact_aliases",
    );

    let fresh_shared = smart_context_test_shared("stable-artifact-alias-persist-fresh");
    register_runtime_smart_context_proxy_state(
        &fresh_shared.log_path,
        true,
        None,
        Some(artifact_path.clone()),
    );
    with_runtime_smart_context_proxy_state(&fresh_shared, |state| {
        let mut value = serde_json::json!({
            "input": [{"type": "function_call_output", "output": format!("psc m refs-only\n{refs}")}]
        });
        assert!(runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
            &mut value,
            state,
        ));
        let output = value["input"][0]["output"].as_str().unwrap();
        assert!(output.contains(&format!(
            "psc a @0={}",
            runtime_smart_context_artifact_ref(&artifact.id)
        )));
        assert!(output.contains("@0#L1-L1"));
    })
    .unwrap();

    let raw = std::fs::read_to_string(&calibration_path).unwrap();
    assert!(raw.contains("\"artifact_aliases\""));
    assert!(!raw.contains("line one"));
    let _ = std::fs::remove_file(&artifact_path);
    let _ = std::fs::remove_file(&calibration_path);
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&artifact_path));
    let _ = std::fs::remove_file(crate::runtime_store::json_lock_file_path(&calibration_path));
}
