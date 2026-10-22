use super::ProfileExportEnvelopeSchemaError;
use crate::{MojoError, rich::ensure_rich_abi};

unsafe extern "C" {
    fn prodex_profile_export_diagnostic_v1(
        version: i64,
        issue: i64,
        expected: i64,
        received: i64,
        iterations: i64,
        argon2: i64,
        output: u64,
        capacity: i64,
        written: u64,
    ) -> i64;
}

/// Render schema diagnostics from numeric metadata only; propagate Mojo transport errors.
pub fn profile_export_schema_diagnostic(
    issue: ProfileExportEnvelopeSchemaError,
    expected_version: Option<u32>,
    received_version: u32,
    iterations: Option<u32>,
    argon2_version: Option<u32>,
) -> Result<String, MojoError> {
    use ProfileExportEnvelopeSchemaError::*;
    let issue = match issue {
        Mojo(error) => return Err(error),
        Pbkdf2Iterations => 6,
        Argon2Version => 7,
        Argon2Memory => 8,
        Argon2Iterations => 9,
        Argon2Parallelism => 10,
        Format => 11,
        Version => 12,
        Cipher => 13,
        Kdf => 14,
        SaltEncoding => 15,
        SaltLength => 16,
        NonceEncoding => 17,
        NonceLength => 18,
        CiphertextEncoding => 19,
        CiphertextLength => 20,
        CiphertextMinimumLength => 21,
        KindVersion => 22,
    };
    ensure_rich_abi()?;
    let mut output = [0_u8; 256];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_profile_export_diagnostic_v1(
            1,
            issue,
            expected_version.map_or(-1, i64::from),
            i64::from(received_version),
            iterations.map_or(-1, i64::from),
            argon2_version.map_or(-1, i64::from),
            output.as_mut_ptr() as u64,
            output.len() as i64,
            &mut written as *mut i64 as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    String::from_utf8(
        output
            .get(..written)
            .ok_or(MojoError::InvalidOutput)?
            .to_vec(),
    )
    .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
