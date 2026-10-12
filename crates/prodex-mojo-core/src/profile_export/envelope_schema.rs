//! Typed ABI boundary for the Mojo-owned export envelope schema.
use super::ProfileImportStringView;
use crate::{MojoError, rich::ensure_rich_abi};

const PROFILE_EXPORT_ENVELOPE_SCHEMA_ABI_VERSION: i64 = 1;

/// Envelope shape supplied to the Mojo-owned profile-export schema validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileExportEnvelopeSchemaInput<'a> {
    Header {
        format: &'a str,
        version: u32,
    },
    Plain {
        format: &'a str,
        version: u32,
    },
    EncryptedV1 {
        format: &'a str,
        version: u32,
        cipher: &'a str,
        kdf: &'a str,
        iterations: u32,
        salt_base64: &'a str,
        nonce_base64: &'a str,
        ciphertext_base64: &'a str,
    },
    EncryptedV2 {
        format: &'a str,
        version: u32,
        cipher: &'a str,
        kdf: &'a str,
        argon2_version: u32,
        argon2_memory_kib: u32,
        argon2_iterations: u32,
        argon2_parallelism: u32,
        salt_base64: &'a str,
        nonce_base64: &'a str,
        ciphertext_base64: &'a str,
    },
}

/// Schema violation returned by the Mojo-owned profile-export validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileExportEnvelopeSchemaError {
    Mojo(MojoError),
    Format,
    Version,
    KindVersion,
    Cipher,
    Kdf,
    SaltEncoding,
    SaltLength,
    NonceEncoding,
    NonceLength,
    CiphertextEncoding,
    CiphertextLength,
    CiphertextMinimumLength,
    Pbkdf2Iterations,
    Argon2Version,
    Argon2Memory,
    Argon2Iterations,
    Argon2Parallelism,
}

unsafe extern "C" {
    fn prodex_profile_export_envelope_schema_v1(abi_version: i64, input_address: u64) -> i64;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ProfileExportEnvelopeSchemaFfiInput {
    kind: i64,
    version: i64,
    format: ProfileImportStringView,
    cipher: ProfileImportStringView,
    kdf: ProfileImportStringView,
    salt_base64: ProfileImportStringView,
    nonce_base64: ProfileImportStringView,
    ciphertext_base64: ProfileImportStringView,
    pbkdf2_iterations: u64,
    argon2_version: u64,
    argon2_memory_kib: u64,
    argon2_iterations: u64,
    argon2_parallelism: u64,
}

const _: () = {
    assert!(std::mem::size_of::<ProfileExportEnvelopeSchemaFfiInput>() == 152);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, format) == 16);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, cipher) == 32);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, kdf) == 48);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, salt_base64) == 64);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, nonce_base64) == 80);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, ciphertext_base64) == 96);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, pbkdf2_iterations) == 112);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, argon2_version) == 120);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, argon2_memory_kib) == 128);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, argon2_iterations) == 136);
    assert!(std::mem::offset_of!(ProfileExportEnvelopeSchemaFfiInput, argon2_parallelism) == 144);
};

fn profile_export_envelope_schema_ffi(
    input: ProfileExportEnvelopeSchemaInput<'_>,
) -> Result<ProfileExportEnvelopeSchemaFfiInput, MojoError> {
    fn base(
        kind: i64,
        format: &str,
        version: u32,
    ) -> Result<ProfileExportEnvelopeSchemaFfiInput, MojoError> {
        Ok(ProfileExportEnvelopeSchemaFfiInput {
            kind,
            version: i64::from(version),
            format: ProfileImportStringView::from(Some(format))?,
            ..Default::default()
        })
    }

    match input {
        ProfileExportEnvelopeSchemaInput::Header { format, version } => base(0, format, version),
        ProfileExportEnvelopeSchemaInput::Plain { format, version } => base(1, format, version),
        ProfileExportEnvelopeSchemaInput::EncryptedV1 {
            format,
            version,
            cipher,
            kdf,
            iterations,
            salt_base64,
            nonce_base64,
            ciphertext_base64,
        } => {
            let mut ffi = base(2, format, version)?;
            ffi.cipher = ProfileImportStringView::from(Some(cipher))?;
            ffi.kdf = ProfileImportStringView::from(Some(kdf))?;
            ffi.salt_base64 = ProfileImportStringView::from(Some(salt_base64))?;
            ffi.nonce_base64 = ProfileImportStringView::from(Some(nonce_base64))?;
            ffi.ciphertext_base64 = ProfileImportStringView::from(Some(ciphertext_base64))?;
            ffi.pbkdf2_iterations = u64::from(iterations);
            Ok(ffi)
        }
        ProfileExportEnvelopeSchemaInput::EncryptedV2 {
            format,
            version,
            cipher,
            kdf,
            argon2_version,
            argon2_memory_kib,
            argon2_iterations,
            argon2_parallelism,
            salt_base64,
            nonce_base64,
            ciphertext_base64,
        } => {
            let mut ffi = base(3, format, version)?;
            ffi.cipher = ProfileImportStringView::from(Some(cipher))?;
            ffi.kdf = ProfileImportStringView::from(Some(kdf))?;
            ffi.salt_base64 = ProfileImportStringView::from(Some(salt_base64))?;
            ffi.nonce_base64 = ProfileImportStringView::from(Some(nonce_base64))?;
            ffi.ciphertext_base64 = ProfileImportStringView::from(Some(ciphertext_base64))?;
            ffi.argon2_version = u64::from(argon2_version);
            ffi.argon2_memory_kib = u64::from(argon2_memory_kib);
            ffi.argon2_iterations = u64::from(argon2_iterations);
            ffi.argon2_parallelism = u64::from(argon2_parallelism);
            Ok(ffi)
        }
    }
}

/// Validate profile-export envelope metadata and encoded fields in Mojo.
pub fn validate_profile_export_envelope_schema(
    input: ProfileExportEnvelopeSchemaInput<'_>,
) -> Result<(), ProfileExportEnvelopeSchemaError> {
    ensure_rich_abi().map_err(ProfileExportEnvelopeSchemaError::Mojo)?;
    let ffi = profile_export_envelope_schema_ffi(input)
        .map_err(ProfileExportEnvelopeSchemaError::Mojo)?;
    let status = unsafe {
        prodex_profile_export_envelope_schema_v1(
            PROFILE_EXPORT_ENVELOPE_SCHEMA_ABI_VERSION,
            (&ffi as *const ProfileExportEnvelopeSchemaFfiInput) as usize as u64,
        )
    };
    match status {
        0 => Ok(()),
        6 => Err(ProfileExportEnvelopeSchemaError::Pbkdf2Iterations),
        7 => Err(ProfileExportEnvelopeSchemaError::Argon2Version),
        8 => Err(ProfileExportEnvelopeSchemaError::Argon2Memory),
        9 => Err(ProfileExportEnvelopeSchemaError::Argon2Iterations),
        10 => Err(ProfileExportEnvelopeSchemaError::Argon2Parallelism),
        11 => Err(ProfileExportEnvelopeSchemaError::Format),
        12 => Err(ProfileExportEnvelopeSchemaError::Version),
        13 => Err(ProfileExportEnvelopeSchemaError::Cipher),
        14 => Err(ProfileExportEnvelopeSchemaError::Kdf),
        15 => Err(ProfileExportEnvelopeSchemaError::SaltEncoding),
        16 => Err(ProfileExportEnvelopeSchemaError::SaltLength),
        17 => Err(ProfileExportEnvelopeSchemaError::NonceEncoding),
        18 => Err(ProfileExportEnvelopeSchemaError::NonceLength),
        19 => Err(ProfileExportEnvelopeSchemaError::CiphertextEncoding),
        20 => Err(ProfileExportEnvelopeSchemaError::CiphertextLength),
        21 => Err(ProfileExportEnvelopeSchemaError::CiphertextMinimumLength),
        22 => Err(ProfileExportEnvelopeSchemaError::KindVersion),
        99 => Err(ProfileExportEnvelopeSchemaError::Mojo(
            MojoError::InvalidInput,
        )),
        100 => Err(ProfileExportEnvelopeSchemaError::Mojo(
            MojoError::AbiMismatch,
        )),
        _ => Err(ProfileExportEnvelopeSchemaError::Mojo(
            MojoError::InvalidOutput,
        )),
    }
}
