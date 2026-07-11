use super::*;
use crate::{ExportedProfile, ExportedSecretFile, ProfileExportPayload};
use std::sync::atomic::{AtomicBool, Ordering};

fn encrypted_envelope(iterations: u32, salt: &[u8]) -> ProfileExportEnvelope<serde_json::Value> {
    ProfileExportEnvelope::Encrypted {
        format: PROFILE_EXPORT_FORMAT.to_string(),
        version: PROFILE_EXPORT_VERSION_V1,
        cipher: PROFILE_EXPORT_CIPHER.to_string(),
        kdf: PROFILE_EXPORT_KDF.to_string(),
        iterations,
        salt_base64: base64::engine::general_purpose::STANDARD.encode(salt),
        nonce_base64: base64::engine::general_purpose::STANDARD
            .encode([0_u8; PROFILE_EXPORT_NONCE_BYTES]),
        ciphertext_base64: String::new(),
    }
}

fn argon2_envelope(
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> ProfileExportEnvelope<serde_json::Value> {
    ProfileExportEnvelope::EncryptedV2 {
        format: PROFILE_EXPORT_FORMAT.to_string(),
        version: PROFILE_EXPORT_VERSION_V2,
        cipher: PROFILE_EXPORT_CIPHER.to_string(),
        kdf: ProfileExportKdfParameters::Argon2id {
            version: PROFILE_EXPORT_ARGON2_VERSION,
            memory_kib,
            iterations,
            parallelism,
        },
        salt_base64: base64::engine::general_purpose::STANDARD
            .encode([0_u8; PROFILE_EXPORT_SALT_BYTES]),
        nonce_base64: base64::engine::general_purpose::STANDARD
            .encode([0_u8; PROFILE_EXPORT_NONCE_BYTES]),
        ciphertext_base64: base64::engine::general_purpose::STANDARD
            .encode([0_u8; PROFILE_EXPORT_AUTH_TAG_BYTES]),
    }
}

fn plain_profile_envelope_json(profiles: Vec<serde_json::Value>) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "payload_kind": "plain",
        "format": PROFILE_EXPORT_FORMAT,
        "version": PROFILE_EXPORT_VERSION_V1,
        "payload": {
            "exported_at": "2025-01-01T00:00:00Z",
            "source_prodex_version": "0.1.0",
            "active_profile": null,
            "profiles": profiles,
        }
    }))
    .expect("test envelope should serialize")
}

fn profile_json(auth_json: String, secret_files: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({
        "name": "fixture",
        "email": "fixture@example.com",
        "source_managed": true,
        "provider": null,
        "auth_json": auth_json,
        "secret_files": secret_files,
    })
}

#[test]
fn encrypted_bundle_rejects_unbounded_kdf_iterations_before_password_lookup() {
    let password_requested = AtomicBool::new(false);
    let err = decode_profile_export_envelope(
        encrypted_envelope(u32::MAX, &[0_u8; PROFILE_EXPORT_SALT_BYTES]),
        || {
            password_requested.store(true, Ordering::SeqCst);
            Ok("secret".to_string())
        },
    )
    .expect_err("unsupported iteration count should fail");

    assert!(err.to_string().contains("PBKDF2 iteration count"));
    assert!(!password_requested.load(Ordering::SeqCst));
}

#[test]
fn version_one_pbkdf2_range_is_bounded_not_exact() {
    assert!(
        validate_profile_export_pbkdf2_iterations(PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS).is_ok()
    );
    assert!(
        validate_profile_export_pbkdf2_iterations(PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS).is_ok()
    );
    assert!(
        validate_profile_export_pbkdf2_iterations(PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS - 1)
            .is_err()
    );
    assert!(
        validate_profile_export_pbkdf2_iterations(PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS + 1)
            .is_err()
    );
}

#[test]
fn encrypted_bundle_rejects_invalid_salt_length_before_password_lookup() {
    let password_requested = AtomicBool::new(false);
    let err = decode_profile_export_envelope(
        encrypted_envelope(crate::PROFILE_EXPORT_PBKDF2_ITERATIONS, &[0_u8; 1]),
        || {
            password_requested.store(true, Ordering::SeqCst);
            Ok("secret".to_string())
        },
    )
    .expect_err("invalid salt should fail");

    assert!(err.to_string().contains("salt length"));
    assert!(!password_requested.load(Ordering::SeqCst));
}

#[test]
fn version_one_known_answer_fixture_remains_decryptable() {
    let envelope = parse_profile_export_envelope::<serde_json::Value>(include_bytes!(
        "../../tests/fixtures/v1-pbkdf2-known-answer.json"
    ))
    .expect("v1 fixture should parse");
    let payload = decode_profile_export_envelope(envelope, || Ok("fixture-password".to_string()))
        .expect("v1 fixture should decrypt");

    assert_eq!(
        payload,
        serde_json::json!({"fixture": "v1", "profiles": []})
    );
}

#[test]
fn encrypted_exports_use_argon2id_v2_and_round_trip() {
    let payload = serde_json::json!({"fixture": "v2"});
    let encoded = serialize_profile_export_payload(&payload, Some("correct-password"))
        .expect("v2 export should encrypt");
    let encoded_text = std::str::from_utf8(&encoded).expect("v2 envelope should be UTF-8");
    assert!(encoded_text.contains("\"payload_kind\": \"encrypted_v2\""));
    assert!(encoded_text.contains("\"algorithm\": \"argon2id\""));
    let envelope = parse_profile_export_envelope::<serde_json::Value>(&encoded)
        .expect("v2 envelope should parse");
    assert!(matches!(
        &envelope,
        ProfileExportEnvelope::EncryptedV2 {
            version: PROFILE_EXPORT_VERSION_V2,
            kdf: ProfileExportKdfParameters::Argon2id {
                version: PROFILE_EXPORT_ARGON2_VERSION,
                memory_kib: PROFILE_EXPORT_ARGON2_MEMORY_KIB,
                iterations: PROFILE_EXPORT_ARGON2_ITERATIONS,
                parallelism: PROFILE_EXPORT_ARGON2_PARALLELISM,
            },
            ..
        }
    ));
    let wrong_password =
        decode_profile_export_envelope(envelope.clone(), || Ok("wrong-password".to_string()))
            .expect_err("wrong password should fail");
    assert!(wrong_password.to_string().contains("failed to decrypt"));
    let decoded = decode_profile_export_envelope(envelope, || Ok("correct-password".to_string()))
        .expect("v2 envelope should decrypt");
    assert_eq!(decoded, payload);
}

#[test]
fn corrupted_v2_ciphertext_fails_authentication() {
    let encoded = serialize_profile_export_payload(
        &serde_json::json!({"fixture": "corrupt"}),
        Some("correct-password"),
    )
    .expect("v2 export should encrypt");
    let mut envelope = parse_profile_export_envelope::<serde_json::Value>(&encoded)
        .expect("v2 envelope should parse");
    let ProfileExportEnvelope::EncryptedV2 {
        ciphertext_base64, ..
    } = &mut envelope
    else {
        panic!("expected v2 envelope");
    };
    let replacement = if ciphertext_base64.starts_with('A') {
        "B"
    } else {
        "A"
    };
    ciphertext_base64.replace_range(..1, replacement);

    let err = decode_profile_export_envelope(envelope, || Ok("correct-password".to_string()))
        .expect_err("corrupt ciphertext should fail");
    assert!(err.to_string().contains("failed to decrypt"));
}

#[test]
fn argon2id_resource_limits_fail_before_password_lookup() {
    let cases = [
        argon2_envelope(
            PROFILE_EXPORT_ARGON2_MAX_MEMORY_KIB + 1,
            PROFILE_EXPORT_ARGON2_ITERATIONS,
            PROFILE_EXPORT_ARGON2_PARALLELISM,
        ),
        argon2_envelope(
            PROFILE_EXPORT_ARGON2_MEMORY_KIB,
            PROFILE_EXPORT_ARGON2_MAX_ITERATIONS + 1,
            PROFILE_EXPORT_ARGON2_PARALLELISM,
        ),
        argon2_envelope(
            PROFILE_EXPORT_ARGON2_MEMORY_KIB,
            PROFILE_EXPORT_ARGON2_ITERATIONS,
            PROFILE_EXPORT_ARGON2_MAX_PARALLELISM + 1,
        ),
    ];
    for envelope in cases {
        let password_requested = AtomicBool::new(false);
        let err = decode_profile_export_envelope(envelope, || {
            password_requested.store(true, Ordering::SeqCst);
            Ok("secret".to_string())
        })
        .expect_err("unsafe Argon2id parameters should fail");
        assert!(err.to_string().contains("Argon2id"));
        assert!(!password_requested.load(Ordering::SeqCst));
    }
}

#[test]
fn ciphertext_and_password_limits_are_bounded() {
    let oversized_ciphertext = base64::engine::general_purpose::STANDARD.encode(vec![
        0_u8;
        PROFILE_EXPORT_CIPHERTEXT_MAX_BYTES
            + 1
    ]);
    let mut envelope = argon2_envelope(
        PROFILE_EXPORT_ARGON2_MEMORY_KIB,
        PROFILE_EXPORT_ARGON2_ITERATIONS,
        PROFILE_EXPORT_ARGON2_PARALLELISM,
    );
    if let ProfileExportEnvelope::EncryptedV2 {
        ciphertext_base64, ..
    } = &mut envelope
    {
        *ciphertext_base64 = oversized_ciphertext;
    }
    let password_requested = AtomicBool::new(false);
    let err = decode_profile_export_envelope(envelope, || {
        password_requested.store(true, Ordering::SeqCst);
        Ok("secret".to_string())
    })
    .expect_err("oversized ciphertext should fail");
    assert!(err.to_string().contains("payload length"));
    assert!(!password_requested.load(Ordering::SeqCst));

    let err = decode_profile_export_envelope(
        argon2_envelope(
            PROFILE_EXPORT_ARGON2_MEMORY_KIB,
            PROFILE_EXPORT_ARGON2_ITERATIONS,
            PROFILE_EXPORT_ARGON2_PARALLELISM,
        ),
        || Ok("x".repeat(PROFILE_EXPORT_PASSWORD_MAX_BYTES + 1)),
    )
    .expect_err("oversized password should fail before KDF work");
    assert!(err.to_string().contains("password"));
}

#[test]
fn malformed_envelopes_are_rejected_without_secret_echo() {
    let err = parse_profile_export_envelope::<serde_json::Value>(b"{not-json")
        .expect_err("malformed envelope should fail");
    let rendered = format!("{err:#}");
    assert!(rendered.contains("invalid profile export envelope"));
    assert!(!rendered.contains("not-json"));

    let unknown_kdf = serde_json::to_vec(&serde_json::json!({
        "payload_kind": "encrypted_v2",
        "format": PROFILE_EXPORT_FORMAT,
        "version": PROFILE_EXPORT_VERSION_V2,
        "cipher": PROFILE_EXPORT_CIPHER,
        "kdf": {"algorithm": "bearer-secret-value"},
        "salt_base64": "",
        "nonce_base64": "",
        "ciphertext_base64": "",
    }))
    .expect("malformed test envelope should serialize");
    let err = parse_profile_export_envelope::<serde_json::Value>(&unknown_kdf)
        .expect_err("unknown KDF should fail");
    let rendered = format!("{err:#}");
    assert_eq!(rendered, "invalid profile export envelope");
    assert!(!rendered.contains("bearer-secret-value"));
}

#[test]
fn profile_and_secret_file_counts_are_bounded_during_deserialization() {
    let profiles = (0..=crate::PROFILE_EXPORT_MAX_PROFILES)
        .map(|_| profile_json(String::new(), Vec::new()))
        .collect();
    let err = parse_profile_export_envelope::<ProfileExportPayload<serde_json::Value>>(
        &plain_profile_envelope_json(profiles),
    )
    .expect_err("too many profiles should fail");
    assert!(format!("{err:#}").contains("profile count limit"));

    let secret_files = (0..=crate::PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE)
        .map(|index| serde_json::json!({"path": format!("secret-{index}.json"), "text": "{}"}))
        .collect();
    let err = parse_profile_export_envelope::<ProfileExportPayload<serde_json::Value>>(
        &plain_profile_envelope_json(vec![profile_json(String::new(), secret_files)]),
    )
    .expect_err("too many secret files should fail");
    assert!(format!("{err:#}").contains("secret-file count limit"));
}

#[test]
fn nested_secret_json_is_bounded() {
    let profiles = vec![profile_json(
        "x".repeat(crate::PROFILE_EXPORT_NESTED_JSON_MAX_BYTES + 1),
        Vec::new(),
    )];
    let err = parse_profile_export_envelope::<ProfileExportPayload<serde_json::Value>>(
        &plain_profile_envelope_json(profiles),
    )
    .expect_err("oversized nested secret should fail");
    assert!(format!("{err:#}").contains("nested secret exceeds size limit"));
}

#[test]
fn secret_debug_output_is_redacted() {
    let payload = ProfileExportPayload {
        exported_at: "2025-01-01T00:00:00Z".to_string(),
        source_prodex_version: "0.1.0".to_string(),
        active_profile: Some("fixture".to_string()),
        profiles: vec![ExportedProfile {
            name: "fixture".to_string(),
            email: Some("fixture@example.com".to_string()),
            source_managed: true,
            provider: serde_json::Value::Null,
            auth_json: "bearer-secret-value".to_string(),
            secret_files: vec![ExportedSecretFile {
                path: "credentials.json".to_string(),
                text: "nested-secret-value".to_string(),
            }],
        }],
    };
    let payload_debug = format!("{payload:?}");
    assert!(!payload_debug.contains("bearer-secret-value"));
    assert!(!payload_debug.contains("nested-secret-value"));
    assert!(payload_debug.contains("<redacted>"));

    let envelope = argon2_envelope(
        PROFILE_EXPORT_ARGON2_MEMORY_KIB,
        PROFILE_EXPORT_ARGON2_ITERATIONS,
        PROFILE_EXPORT_ARGON2_PARALLELISM,
    );
    let envelope_debug = format!("{envelope:?}");
    assert!(!envelope_debug.contains("AAAAAAAA"));
    assert!(envelope_debug.contains("<redacted>"));
}
