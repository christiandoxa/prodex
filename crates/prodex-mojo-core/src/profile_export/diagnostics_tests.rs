use super::*;

#[test]
fn compiled_schema_diagnostics_preserve_all_messages() {
    use ProfileExportEnvelopeSchemaError::*;
    for (issue, expected) in [
        (Format, "unsupported profile export format"),
        (
            Version,
            "unsupported profile export version 77 (supported: 1, 2)",
        ),
        (
            KindVersion,
            "profile export envelope kind requires version 2 (received 77)",
        ),
        (Cipher, "unsupported profile export cipher"),
        (Kdf, "unsupported profile export KDF"),
        (SaltEncoding, "invalid encrypted export salt encoding"),
        (SaltLength, "invalid encrypted export salt length"),
        (NonceEncoding, "invalid encrypted export nonce encoding"),
        (NonceLength, "invalid encrypted export nonce length"),
        (
            CiphertextEncoding,
            "invalid encrypted export payload encoding",
        ),
        (CiphertextLength, "invalid encrypted export payload length"),
        (
            CiphertextMinimumLength,
            "invalid encrypted export payload length",
        ),
        (
            Pbkdf2Iterations,
            "profile export PBKDF2 iteration count 23 is outside safe range 50000..=2000000",
        ),
        (
            Argon2Version,
            "unsupported profile export Argon2id version 99",
        ),
        (
            Argon2Memory,
            "profile export Argon2id memory cost is outside safe range",
        ),
        (
            Argon2Iterations,
            "profile export Argon2id iteration count is outside safe range",
        ),
        (
            Argon2Parallelism,
            "profile export Argon2id parallelism is outside safe range",
        ),
    ] {
        assert_eq!(
            profile_export_schema_diagnostic(issue, Some(2), 77, Some(23), Some(99)).unwrap(),
            expected
        );
    }
}

#[test]
fn compiled_schema_diagnostics_preserve_absence_extrema_and_transport_errors() {
    use ProfileExportEnvelopeSchemaError::*;
    for (issue, expected) in [
        (KindVersion, "invalid profile export envelope version"),
        (
            Pbkdf2Iterations,
            "profile export PBKDF2 iteration count 0 is outside safe range 50000..=2000000",
        ),
        (
            Argon2Version,
            "unsupported profile export Argon2id version 0",
        ),
        (
            Version,
            "unsupported profile export version 4294967295 (supported: 1, 2)",
        ),
    ] {
        assert_eq!(
            profile_export_schema_diagnostic(issue, None, u32::MAX, None, None).unwrap(),
            expected
        );
    }
    for error in [
        MojoError::InvalidInput,
        MojoError::AbiMismatch,
        MojoError::Capacity,
    ] {
        assert_eq!(
            profile_export_schema_diagnostic(Mojo(error), None, 0, None, None),
            Err(error)
        );
    }
}

#[test]
fn schema_diagnostic_abi_rejects_invalid_metadata_and_capacity_without_output() {
    let mut output = [73_u8; 256];
    let mut written = 83_i64;
    let result = &mut written as *mut i64 as u64;
    let mut call = |version, issue, expected, received, iterations, argon2, capacity, result| unsafe {
        prodex_profile_export_diagnostic_v1(
            version,
            issue,
            expected,
            received,
            iterations,
            argon2,
            output.as_mut_ptr() as u64,
            capacity,
            result,
        )
    };
    assert_eq!(call(2, 6, -1, 0, -1, -1, 256, result), 4);
    for issue in [5, 23, 99, 100] {
        assert_eq!(call(1, issue, -1, 0, -1, -1, 256, result), 1);
    }
    for (expected, received, iterations, argon2) in [
        (-2, 0, -1, -1),
        (-1, -1, -1, -1),
        (-1, 0, -2, -1),
        (-1, 0, -1, -2),
        (4294967296, 0, -1, -1),
        (-1, 4294967296, -1, -1),
        (-1, 0, 4294967296, -1),
        (-1, 0, -1, 4294967296),
    ] {
        assert_eq!(
            call(1, 6, expected, received, iterations, argon2, 256, result),
            1
        );
    }
    assert_eq!(call(1, 6, -1, 0, -1, -1, -1, result), 1);
    assert_eq!(call(1, 6, -1, 0, -1, -1, 1, result), 3);
    assert_eq!(call(1, 6, -1, 0, -1, -1, 256, 0), 1);
    assert_eq!(call(1, 6, -1, 0, -1, -1, 256, result + 1), 1);
    assert_eq!(output, [73; 256]);
    assert_eq!(written, 83);
}
