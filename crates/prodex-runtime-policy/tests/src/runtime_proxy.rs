use super::*;

#[test]
fn load_runtime_policy_from_root_parses_runtime_proxy_preset() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-parse");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "many-terminals"
"#,
    )
    .unwrap();

    let loaded = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(
        loaded.runtime_proxy.preset().map(|preset| preset.as_str()),
        Some("many-terminals")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_padded_runtime_proxy_preset() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-exact");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = " many-terminals "
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    let detail = format!("{err:#}");
    assert!(detail.contains("unknown variant"), "{detail}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_proxy_applies_preset_values_and_explicit_overrides() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-values");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "many-terminals"
active_request_limit = 99
"#,
    )
    .unwrap();
    let loaded = runtime_policy_proxy_from_root(&root, None).unwrap();
    assert_eq!(
        loaded.preset().map(|preset| preset.as_str()),
        Some("many-terminals")
    );
    assert_eq!(loaded.worker_count, Some(12));
    assert_eq!(loaded.long_lived_worker_count, Some(32));
    assert_eq!(loaded.long_lived_queue_capacity, Some(512));
    assert_eq!(loaded.active_request_limit, Some(99));
    assert_eq!(loaded.responses_active_limit, Some(120));
    assert_eq!(loaded.websocket_active_limit, Some(32));
    assert_eq!(loaded.websocket_connect_overflow_capacity, Some(384));

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[cfg(feature = "mojo")]
fn runtime_proxy_preset_values(
    settings: &crate::RuntimePolicyProxySettings,
) -> [Option<usize>; 19] {
    [
        settings.worker_count,
        settings.long_lived_worker_count,
        settings.probe_refresh_worker_count,
        settings.async_worker_count,
        settings.long_lived_queue_capacity,
        settings.active_request_limit,
        settings.profile_inflight_soft_limit,
        settings.profile_inflight_hard_limit,
        settings.responses_active_limit,
        settings.compact_active_limit,
        settings.websocket_active_limit,
        settings.standard_active_limit,
        settings.websocket_connect_worker_count,
        settings.websocket_connect_queue_capacity,
        settings.websocket_connect_overflow_capacity,
        settings.websocket_dns_worker_count,
        settings.websocket_dns_queue_capacity,
        settings.websocket_dns_overflow_capacity,
        settings.startup_sync_probe_warm_limit,
    ]
}

#[cfg(feature = "mojo")]
fn rust_runtime_proxy_preset_values(preset: RuntimePolicyProxyPreset) -> [Option<usize>; 19] {
    let mut values = [None; 19];
    match preset {
        RuntimePolicyProxyPreset::Low => {
            values = [
                Some(4),
                Some(8),
                Some(2),
                Some(2),
                Some(128),
                Some(48),
                Some(2),
                Some(4),
                Some(36),
                Some(3),
                Some(8),
                Some(2),
                Some(4),
                Some(32),
                Some(64),
                Some(2),
                Some(16),
                Some(32),
                Some(1),
            ]
        }
        RuntimePolicyProxyPreset::Default => {}
        RuntimePolicyProxyPreset::ManyTerminals => {
            values = [
                Some(12),
                Some(32),
                Some(4),
                Some(4),
                Some(512),
                Some(160),
                Some(4),
                Some(8),
                Some(120),
                Some(8),
                Some(32),
                Some(8),
                Some(12),
                Some(96),
                Some(384),
                Some(6),
                Some(48),
                Some(96),
                Some(2),
            ]
        }
        RuntimePolicyProxyPreset::Aggressive => {
            values = [
                Some(24),
                Some(96),
                Some(8),
                Some(8),
                Some(1024),
                Some(384),
                Some(8),
                Some(16),
                Some(288),
                Some(16),
                Some(96),
                Some(16),
                Some(16),
                Some(128),
                Some(512),
                Some(8),
                Some(64),
                Some(128),
                Some(3),
            ]
        }
    }
    values
}

#[cfg(feature = "mojo")]
#[test]
fn mojo_runtime_proxy_preset_normalization_matches_rust_oracle() {
    for preset in [
        RuntimePolicyProxyPreset::Low,
        RuntimePolicyProxyPreset::Default,
        RuntimePolicyProxyPreset::ManyTerminals,
        RuntimePolicyProxyPreset::Aggressive,
    ] {
        clear_runtime_policy_cache();
        let root = temp_root("preset-mojo-parity");
        let path = runtime_policy_path(&root);
        fs::write(
            &path,
            format!(
                "version = 1\n\n[runtime_proxy]\npreset = \"{}\"\n",
                preset.as_str()
            ),
        )
        .unwrap();

        let loaded = runtime_policy_proxy_from_root(&root, None).unwrap();
        assert_eq!(
            runtime_proxy_preset_values(&loaded),
            rust_runtime_proxy_preset_values(preset),
            "preset {}",
            preset.as_str()
        );

        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn runtime_policy_proxy_uses_env_preset_without_policy_file() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-env-no-file");

    let loaded =
        runtime_policy_proxy_from_root(&root, RuntimePolicyProxyPreset::parse("low")).unwrap();
    assert_eq!(loaded.preset().map(|preset| preset.as_str()), Some("low"));
    assert_eq!(loaded.worker_count, Some(4));
    assert_eq!(loaded.active_request_limit, Some(48));
    assert_eq!(loaded.profile_inflight_hard_limit, Some(4));

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_proxy_default_preset_keeps_tuning_values_unset() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-default");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "default"
"#,
    )
    .unwrap();
    let loaded = runtime_policy_proxy_from_root(&root, None).unwrap();
    assert_eq!(
        loaded.preset().map(|preset| preset.as_str()),
        Some("default")
    );
    assert_eq!(loaded.worker_count, None);
    assert_eq!(loaded.long_lived_worker_count, None);
    assert_eq!(loaded.active_request_limit, None);
    assert_eq!(loaded.responses_active_limit, None);

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_proxy_env_preset_overrides_configured_preset() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-env-override");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "low"
"#,
    )
    .unwrap();
    let loaded =
        runtime_policy_proxy_from_root(&root, RuntimePolicyProxyPreset::parse("aggressive"))
            .unwrap();
    assert_eq!(
        loaded.preset().map(|preset| preset.as_str()),
        Some("aggressive")
    );
    assert_eq!(loaded.worker_count, Some(24));
    assert_eq!(loaded.long_lived_worker_count, Some(96));
    assert_eq!(loaded.active_request_limit, Some(384));
    assert_eq!(loaded.websocket_dns_overflow_capacity, Some(128));

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_unknown_preset() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-unknown");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "huge"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("failed to parse"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_policy_proxy_ignores_unknown_env_preset_and_falls_back_to_config() {
    clear_runtime_policy_cache();
    let root = temp_root("preset-env-unknown");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
preset = "low"
"#,
    )
    .unwrap();
    let loaded =
        runtime_policy_proxy_from_root(&root, RuntimePolicyProxyPreset::parse("huge")).unwrap();
    assert_eq!(loaded.preset().map(|preset| preset.as_str()), Some("low"));
    assert_eq!(loaded.worker_count, Some(4));
    assert_eq!(loaded.active_request_limit, Some(48));

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_unsupported_version() {
    clear_runtime_policy_cache();
    let root = temp_root("version");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 2

[runtime]
log_format = "json"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("unsupported prodex policy version")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_accepts_keyring_secret_backend() {
    clear_runtime_policy_cache();
    let root = temp_root("keyring-secret-backend");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[secrets]
backend = "keyring"
keyring_service = "prodex-test"
"#,
    )
    .unwrap();

    let policy = load_runtime_policy_from_root(&root).unwrap().unwrap();
    assert_eq!(
        policy.secrets.backend,
        Some(secret_store::SecretBackendKind::Keyring)
    );
    assert_eq!(
        policy.secrets.keyring_service.as_deref(),
        Some("prodex-test")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_incomplete_keyring_secret_backend() {
    clear_runtime_policy_cache();
    let root = temp_root("incomplete-keyring-secret-backend");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[secrets]
backend = "keyring"
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(err.to_string().contains("keyring_service"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_runtime_policy_from_root_rejects_zero_profile_inflight_limits() {
    clear_runtime_policy_cache();
    let root = temp_root("inflight-zero");
    let path = runtime_policy_path(&root);
    fs::write(
        &path,
        r#"
version = 1

[runtime_proxy]
profile_inflight_soft_limit = 0
profile_inflight_hard_limit = 1
"#,
    )
    .unwrap();

    let err = load_runtime_policy_from_root(&root).unwrap_err();
    assert!(
        err.to_string()
            .contains("runtime_proxy.profile_inflight_soft_limit")
    );

    let _ = fs::remove_dir_all(root);
}
