use super::*;
use prodex_mojo_core::profile_export::{
    ProfileExportEnvelopeSchemaError as Issue, ProfileExportEnvelopeSchemaInput as Input,
};

#[test]
fn schema_diagnostics_preserve_all_exact_messages_and_numeric_metadata() {
    for (issue, expected) in [
        (Issue::Format, "unsupported profile export format"),
        (
            Issue::Version,
            "unsupported profile export version 77 (supported: 1, 2)",
        ),
        (
            Issue::KindVersion,
            "profile export envelope kind requires version 2 (received 77)",
        ),
        (Issue::Cipher, "unsupported profile export cipher"),
        (Issue::Kdf, "unsupported profile export KDF"),
        (
            Issue::SaltEncoding,
            "invalid encrypted export salt encoding",
        ),
        (Issue::SaltLength, "invalid encrypted export salt length"),
        (
            Issue::NonceEncoding,
            "invalid encrypted export nonce encoding",
        ),
        (Issue::NonceLength, "invalid encrypted export nonce length"),
        (
            Issue::CiphertextEncoding,
            "invalid encrypted export payload encoding",
        ),
        (
            Issue::CiphertextLength,
            "invalid encrypted export payload length",
        ),
        (
            Issue::CiphertextMinimumLength,
            "invalid encrypted export payload length",
        ),
        (
            Issue::Pbkdf2Iterations,
            "profile export PBKDF2 iteration count 23 is outside safe range 50000..=2000000",
        ),
        (
            Issue::Argon2Version,
            "unsupported profile export Argon2id version 99",
        ),
        (
            Issue::Argon2Memory,
            "profile export Argon2id memory cost is outside safe range",
        ),
        (
            Issue::Argon2Iterations,
            "profile export Argon2id iteration count is outside safe range",
        ),
        (
            Issue::Argon2Parallelism,
            "profile export Argon2id parallelism is outside safe range",
        ),
    ] {
        assert_eq!(
            profile_export_schema_error(issue, Some(2), 77, Some(23), Some(99)).to_string(),
            expected
        );
    }
    assert_eq!(
        profile_export_schema_error(Issue::KindVersion, None, 77, None, None).to_string(),
        "invalid profile export envelope version"
    );
    assert_eq!(
        profile_export_schema_error(Issue::Pbkdf2Iterations, None, 0, None, None).to_string(),
        "profile export PBKDF2 iteration count 0 is outside safe range 50000..=2000000"
    );
    assert_eq!(
        profile_export_schema_error(Issue::Argon2Version, None, 0, None, None).to_string(),
        "unsupported profile export Argon2id version 0"
    );
    assert_eq!(
        profile_export_schema_error(Issue::Version, None, u32::MAX, None, None).to_string(),
        "unsupported profile export version 4294967295 (supported: 1, 2)"
    );
    assert_eq!(
        profile_export_schema_error(
            Issue::Mojo(prodex_mojo_core::MojoError::InvalidInput),
            None,
            0,
            None,
            None
        )
        .to_string(),
        "Mojo profile-export envelope validation failed: InvalidInput"
    );
}

#[test]
fn real_schema_callers_preserve_header_and_envelope_error_precedence() {
    assert_eq!(
        validate_profile_export_header("invalid-format", 77)
            .unwrap_err()
            .to_string(),
        "unsupported profile export format"
    );
    assert_eq!(
        validate_profile_export_header(PROFILE_EXPORT_FORMAT, 77)
            .unwrap_err()
            .to_string(),
        "unsupported profile export version 77 (supported: 1, 2)"
    );
    assert_eq!(
        validate_profile_export_schema_input(Input::Plain {
            format: PROFILE_EXPORT_FORMAT,
            version: 2
        })
        .unwrap_err()
        .to_string(),
        "profile export envelope kind requires version 1 (received 2)"
    );
    assert_eq!(
        validate_profile_export_schema_input(Input::EncryptedV1 {
            format: PROFILE_EXPORT_FORMAT,
            version: 2,
            cipher: "invalid",
            kdf: "invalid",
            iterations: 0,
            salt_base64: "invalid",
            nonce_base64: "invalid",
            ciphertext_base64: "invalid",
        })
        .unwrap_err()
        .to_string(),
        "profile export envelope kind requires version 1 (received 2)"
    );
}
