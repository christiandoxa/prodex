use super::*;
use anyhow::{Context, bail};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeSmartContextSelfTestStatus {
    pub(crate) tokenizer_family: &'static str,
    pub(crate) detail: String,
}

pub(crate) fn runtime_smart_context_offline_self_test()
-> anyhow::Result<RuntimeSmartContextSelfTestStatus> {
    runtime_smart_context_self_test_exact_and_rollout()?;
    let tokenizer_family = runtime_smart_context_self_test_transform()?;
    runtime_smart_context_self_test_persistence()?;
    Ok(RuntimeSmartContextSelfTestStatus {
        tokenizer_family,
        detail: format!(
            "exact=byte-identical; canary=ok; transform=round-trip; digest=sha256; scope=isolated; persistence=encrypted; corruption=detected; tokenizer={tokenizer_family}; replay_evidence=not-runtime-readiness"
        ),
    })
}

fn runtime_smart_context_self_test_exact_and_rollout() -> anyhow::Result<()> {
    let body = br#"{ "model": "gpt-5.4", "input": [] }"#.to_vec();
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/responses".to_string(),
        headers: vec![("x-prodex-smart-context".to_string(), "exact".to_string())],
        body,
    };
    let Some(Cow::Borrowed(exact)) = runtime_smart_context_exact_passthrough(&request) else {
        bail!("explicit exact mode did not return a borrowed request body");
    };
    if exact != request.body.as_slice() {
        bail!("explicit exact mode changed request bytes");
    }
    for (percent, expected) in [
        (0, runtime_proxy_crate::SmartContextRolloutMode::Disabled),
        (100, runtime_proxy_crate::SmartContextRolloutMode::Apply),
    ] {
        let decision = runtime_proxy_crate::smart_context_rollout_decision(
            runtime_proxy_crate::SmartContextRolloutDecisionInput {
                enabled: true,
                explicit_exact_mode: false,
                shadow_mode: false,
                canary_percent: percent,
                stable_key: "doctor/session/profile/workspace".to_string(),
            },
        );
        if decision.mode != expected || decision.canary_bucket >= 10_000 {
            bail!("canary boundary self-test failed at {percent}%");
        }
    }
    Ok(())
}

fn runtime_smart_context_self_test_transform() -> anyhow::Result<&'static str> {
    let repeated = format!(
        "error: synthetic build failed at src/lib.rs:10:5\n{}",
        "diagnostic line\n".repeat(800)
    );
    let original = serde_json::json!({
        "model": "gpt-5.4",
        "input": [
            {"type": "function_call_output", "call_id": "call_1", "output": repeated},
            {"type": "function_call_output", "call_id": "call_2", "output": repeated}
        ]
    });
    let mut candidate = original.clone();
    let mut stats = RuntimeSmartContextTransformStats::default();
    runtime_smart_context_dedupe_input_text_within_request(&mut candidate, &mut stats);
    runtime_smart_context_append_inline_reference_protocol(&mut candidate, &stats);
    if stats.duplicate_texts != 1 {
        bail!("known duplicate transform did not apply exactly once");
    }
    let expanded = runtime_smart_context_expand_inline_references(&original, &candidate)
        .context("inline reference did not resolve")?;
    let expanded_input = expanded["input"]
        .as_array()
        .context("expanded input is not an array")?;
    let original_input = original["input"]
        .as_array()
        .context("original input is not an array")?;
    if expanded_input.get(..original_input.len()) != Some(original_input.as_slice()) {
        bail!("inline reference round trip changed input content");
    }

    let original_body = serde_json::to_vec(&original)?;
    let candidate_body = serde_json::to_vec(&candidate)?;
    let before = runtime_proxy_crate::smart_context_count_serialized_request(
        &original_body,
        Some("gpt-5.4"),
    );
    let after = runtime_proxy_crate::smart_context_count_serialized_request(
        &candidate_body,
        Some("gpt-5.4"),
    );
    let family = before
        .tokenizer_family
        .filter(|_| before.is_proven() && after.is_proven())
        .context("known model tokenizer is unavailable")?;
    let check = runtime_smart_context_regression_self_check(
        &original_body,
        &candidate_body,
        &before,
        &after,
        runtime_smart_context_critical_signal_self_check(
            &original_body,
            &serde_json::to_vec(&expanded)?,
        ),
        runtime_proxy_crate::smart_context_exactness_guard(
            runtime_proxy_crate::SmartContextExactnessInput::default(),
        ),
        Vec::new(),
    );
    if check.decision != runtime_proxy_crate::SmartContextRegressionSelfCheckDecision::Pass {
        bail!("known transform failed token/correctness validation");
    }

    let digest = runtime_proxy_crate::smart_context_hash_text("doctor artifact");
    if !digest.starts_with("sc2:")
        || !runtime_proxy_crate::smart_context_hash_matches_text(&digest, "doctor artifact")
        || runtime_proxy_crate::smart_context_hash_matches_text(&digest, "tampered")
    {
        bail!("strong digest validation failed");
    }
    Ok(family)
}

fn runtime_smart_context_self_test_persistence() -> anyhow::Result<()> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-smart-context-doctor-{}-{unique}",
        std::process::id()
    ));
    let result = (|| {
        let scope = runtime_proxy_crate::ContextScopeId::new(
            "doctor-tenant",
            "doctor-profile",
            "doctor-provider",
            "/home/test-user/doctor-workspace",
            Some("doctor-session"),
        );
        let other_scope = runtime_proxy_crate::ContextScopeId::new(
            "doctor-tenant",
            "other-profile",
            "doctor-provider",
            "/home/test-user/doctor-workspace",
            Some("doctor-session"),
        );
        let path = root
            .join("smart-context")
            .join("scopes")
            .join(scope.path_component())
            .join("artifacts.json");
        let mut store = RuntimeSmartContextArtifactStore::for_scope(scope.clone());
        let artifact = store
            .insert_text("doctor artifact")
            .context("artifact insertion failed")?;
        store.save_merged_to_path(&path)?;
        let encoded = fs::read(&path)?;
        if encoded
            .windows("doctor artifact".len())
            .any(|window| window == b"doctor artifact")
        {
            bail!("scoped artifact persistence is not encrypted");
        }
        let loaded = RuntimeSmartContextArtifactStore::load_scoped_from_path(&path, &scope)?;
        if loaded.get_text(&artifact.id).as_deref() != Some("doctor artifact") {
            bail!("scoped artifact persistence round trip failed");
        }
        if RuntimeSmartContextArtifactStore::load_scoped_from_path(&path, &other_scope).is_ok() {
            bail!("artifact store accepted the wrong scope");
        }
        fs::write(&path, b"corrupt")?;
        if RuntimeSmartContextArtifactStore::load_scoped_from_path(&path, &scope).is_ok() {
            bail!("corrupt artifact store was accepted");
        }
        Ok(())
    })();
    let _ = fs::remove_dir_all(&root);
    result
}

#[cfg(test)]
#[test]
fn offline_self_test_covers_runtime_readiness_without_network() {
    let status = runtime_smart_context_offline_self_test().expect("offline self-test");
    assert_eq!(status.tokenizer_family, "o200k_base");
    assert!(status.detail.contains("corruption=detected"));
}
