use super::super::*;

#[test]
fn smart_context_condenses_completed_tool_call_arguments() {
    let arguments = serde_json::json!({
        "command": "python3",
        "script": "print('large historical argument')\n".repeat(160),
    });
    let argument_text = serde_json::to_string(&arguments).unwrap();
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "arguments": arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "completed"
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_historical_tool_call_arguments(
        &mut value,
        &mut store,
        9,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        8 * 1024,
        &mut stats,
    );

    let replacement = value["input"][0]["arguments"].as_str().unwrap();
    assert!(replacement.starts_with("psc args psc:"));
    assert!(replacement.contains("b="));
    assert!(replacement.contains("p:"));
    assert!(replacement.len().saturating_mul(4) < argument_text.len());
    assert!(store.artifact_ref_for_exact_text(&argument_text).is_some());
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_call_args_condensed, 1);
}

#[test]
fn smart_context_repeated_tool_call_arguments_use_short_repeat_ref() {
    let arguments = serde_json::json!({
        "cmd": "python3",
        "script": "print('same repeated historical argument')\n".repeat(180),
    });
    let argument_text = serde_json::to_string(&arguments).unwrap();
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "arguments": arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "completed"
            },
            {
                "type": "function_call",
                "call_id": "call_2",
                "arguments": serde_json::from_str::<serde_json::Value>(&argument_text).unwrap()
            },
            {
                "type": "function_call_output",
                "call_id": "call_2",
                "output": "completed"
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_historical_tool_call_arguments(
        &mut value,
        &mut store,
        9,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        8 * 1024,
        &mut stats,
    );

    let first = value["input"][0]["arguments"].as_str().unwrap();
    let second = value["input"][2]["arguments"].as_str().unwrap();
    assert!(first.starts_with("psc args psc:"));
    assert!(second.starts_with("psc args rep psc:"));
    assert!(!second.contains("same repeated historical argument"));
    assert!(second.len() < first.len());
    assert!(store.artifact_ref_for_exact_text(&argument_text).is_some());
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_call_args_condensed, 2);
}

#[test]
fn smart_context_similar_tool_call_arguments_use_delta_ref() {
    let common_prefix = "let shared = 1;\n".repeat(160);
    let common_suffix = "println!(\"done\");\n".repeat(120);
    let first_script = format!("{common_prefix}println!(\"alpha\");\n{common_suffix}");
    let second_script = format!("{common_prefix}println!(\"beta\");\n{common_suffix}");
    let first_arguments = serde_json::json!({
        "cmd": "python3",
        "script": first_script,
    });
    let second_arguments = serde_json::json!({
        "cmd": "python3",
        "script": second_script,
    });
    let first_text = serde_json::to_string(&first_arguments).unwrap();
    let second_text = serde_json::to_string(&second_arguments).unwrap();
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "arguments": first_arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "completed"
            },
            {
                "type": "function_call",
                "call_id": "call_2",
                "arguments": second_arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_2",
                "output": "completed"
            }
        ]
    });
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_historical_tool_call_arguments(
        &mut value,
        &mut store,
        10,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        8 * 1024,
        &mut stats,
    );

    let first = value["input"][0]["arguments"].as_str().unwrap();
    let second = value["input"][2]["arguments"].as_str().unwrap();
    assert!(first.starts_with("psc args psc:"));
    assert!(second.starts_with("psc args d psc:"));
    assert!(second.contains(" base=psc:"));
    assert!(second.contains(" pre="));
    assert!(second.contains(" suf="));
    assert!(second.contains(" ih=sc:"));
    assert!(!second.contains(&common_prefix));
    assert!(!second.contains(&common_suffix));
    assert!(second.len().saturating_mul(4) < second_text.len());
    assert!(store.artifact_ref_for_exact_text(&first_text).is_some());
    assert!(store.artifact_ref_for_exact_text(&second_text).is_some());
    assert_eq!(stats.artifacts_stored, 2);
    assert_eq!(stats.tool_call_args_condensed, 2);
}

#[test]
fn smart_context_keeps_active_tool_call_arguments_exact() {
    let completed_arguments = serde_json::json!({
        "cmd": "python3",
        "script": "print('completed historical argument')\n".repeat(180),
    });
    let active_arguments = serde_json::json!({
        "cmd": "python3",
        "script": "print('active current argument must remain exact')\n".repeat(180),
    });
    let mut value = serde_json::json!({
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "arguments": completed_arguments
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "completed"
            },
            {
                "type": "function_call",
                "call_id": "call_active",
                "arguments": active_arguments
            }
        ]
    });
    let original_active = value["input"][2]["arguments"].clone();
    let mut store = RuntimeSmartContextArtifactStore::default();
    let mut stats = RuntimeSmartContextTransformStats::default();

    runtime_smart_context_condense_historical_tool_call_arguments(
        &mut value,
        &mut store,
        11,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal,
        8 * 1024,
        &mut stats,
    );

    assert!(value["input"][0]["arguments"].as_str().is_some());
    assert_eq!(value["input"][2]["arguments"], original_active);
    assert_eq!(stats.artifacts_stored, 1);
    assert_eq!(stats.tool_call_args_condensed, 1);
}
