const RUNTIME_CONFIG_SCALAR_ABI_VERSION: i64 = 1;
const RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_UNSIGNED: i64 = 0;
const RUNTIME_CONFIG_SCALAR_STRICT_UNSIGNED: i64 = 1;
const RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_SIGNED: i64 = 2;
const RUNTIME_CONFIG_SCALAR_STRICT_BOOL: i64 = 3;
const RUNTIME_CONFIG_SCALAR_COMPAT_UNSIGNED: i64 = 4;
const RUNTIME_CONFIG_SCALAR_COMPAT_FLAG: i64 = 5;
const RUNTIME_CONFIG_SCALAR_COMPAT_BOOL_DEFAULT_TRUE: i64 = 6;
const RUNTIME_CONFIG_SCALAR_FLAG_TRIM: i64 = 1;
const RUNTIME_CONFIG_SCALAR_FLAG_ALLOW_ZERO: i64 = 2;
const RUNTIME_CONFIG_SCALAR_FLAG_CLAMP: i64 = 4;

/// The deterministic scalar operation selected by the runtime configuration caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeConfigScalarKind {
    StrictPositiveUnsigned,
    StrictUnsigned,
    StrictPositiveSigned,
    StrictBool,
    CompatibilityUnsigned,
    CompatibilityFlag,
    CompatibilityBoolDefaultTrue,
}

impl RuntimeConfigScalarKind {
    const fn operation(self) -> i64 {
        match self {
            Self::StrictPositiveUnsigned => RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_UNSIGNED,
            Self::StrictUnsigned => RUNTIME_CONFIG_SCALAR_STRICT_UNSIGNED,
            Self::StrictPositiveSigned => RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_SIGNED,
            Self::StrictBool => RUNTIME_CONFIG_SCALAR_STRICT_BOOL,
            Self::CompatibilityUnsigned => RUNTIME_CONFIG_SCALAR_COMPAT_UNSIGNED,
            Self::CompatibilityFlag => RUNTIME_CONFIG_SCALAR_COMPAT_FLAG,
            Self::CompatibilityBoolDefaultTrue => RUNTIME_CONFIG_SCALAR_COMPAT_BOOL_DEFAULT_TRUE,
        }
    }
}

/// A borrowed environment scalar view. Invalid Unicode is represented without
/// copying or exposing the underlying bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfigScalarInput<'a> {
    bytes: Option<&'a [u8]>,
    valid_utf8: bool,
}

impl<'a> RuntimeConfigScalarInput<'a> {
    /// Create an absent environment value.
    pub const fn absent() -> Self {
        Self {
            bytes: None,
            valid_utf8: true,
        }
    }

    /// Borrow a validated UTF-8 environment value.
    pub fn utf8(value: &'a str) -> Self {
        Self {
            bytes: Some(value.as_bytes()),
            valid_utf8: true,
        }
    }

    /// Represent a present value that failed the host UTF-8 boundary check.
    pub const fn invalid_unicode() -> Self {
        Self {
            bytes: None,
            valid_utf8: false,
        }
    }

    fn abi_parts(self) -> Result<(i64, i64, u64, i64), crate::MojoError> {
        match self.bytes {
            Some(bytes) => Ok((
                1,
                i64::from(self.valid_utf8),
                bytes.as_ptr() as usize as u64,
                i64::try_from(bytes.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            )),
            None => Ok((
                if self.valid_utf8 { 0 } else { 1 },
                i64::from(self.valid_utf8),
                0,
                0,
            )),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct RuntimeConfigScalarRecordV1 {
    operation: i64,
    value_address: u64,
    value_length: i64,
    value_present: i64,
    value_valid_utf8: i64,
    policy_present: i64,
    policy_unsigned: u64,
    default_unsigned: u64,
    minimum_unsigned: u64,
    maximum_unsigned: u64,
    policy_signed: i64,
    default_signed: i64,
    minimum_signed: i64,
    maximum_signed: i64,
    flags: i64,
    output_unsigned: u64,
    output_signed: i64,
}

const _: () = {
    assert!(std::mem::size_of::<RuntimeConfigScalarRecordV1>() == 136);
    assert!(std::mem::align_of::<RuntimeConfigScalarRecordV1>() == 8);
    assert!(std::mem::offset_of!(RuntimeConfigScalarRecordV1, output_unsigned) == 120);
    assert!(std::mem::offset_of!(RuntimeConfigScalarRecordV1, output_signed) == 128);
};

/// Status returned by the Mojo runtime configuration scalar plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeConfigScalarStatus {
    Selected,
    Missing,
    InvalidUnicode,
    Empty,
    Whitespace,
    UnsignedInteger,
    NotPositive,
    PositiveInteger,
    InvalidBool,
    CompatibilityDefault,
}

impl RuntimeConfigScalarStatus {
    fn from_i64(value: i64) -> Result<Self, crate::MojoError> {
        match value {
            0 => Ok(Self::Selected),
            1 => Ok(Self::Missing),
            2 => Ok(Self::InvalidUnicode),
            3 => Ok(Self::Empty),
            4 => Ok(Self::Whitespace),
            5 => Ok(Self::UnsignedInteger),
            6 => Ok(Self::NotPositive),
            7 => Ok(Self::PositiveInteger),
            8 => Ok(Self::InvalidBool),
            9 => Ok(Self::CompatibilityDefault),
            _ => Err(crate::MojoError::InvalidOutput),
        }
    }
}

unsafe extern "C" {
    fn prodex_runtime_config_scalar_v1(
        abi_version: i64,
        record_address: u64,
        record_size: i64,
    ) -> i64;
}

fn runtime_config_scalar_call(
    kind: RuntimeConfigScalarKind,
    input: RuntimeConfigScalarInput<'_>,
    policy_unsigned: Option<u64>,
    default_unsigned: u64,
    minimum_unsigned: u64,
    maximum_unsigned: u64,
    policy_signed: Option<i64>,
    default_signed: i64,
    minimum_signed: i64,
    maximum_signed: i64,
    flags: i64,
) -> Result<(u64, i64, RuntimeConfigScalarStatus), crate::MojoError> {
    let (value_present, value_valid_utf8, value_address, value_length) = input.abi_parts()?;
    let mut record = RuntimeConfigScalarRecordV1 {
        operation: kind.operation(),
        value_address,
        value_length,
        value_present,
        value_valid_utf8,
        policy_present: i64::from(policy_unsigned.is_some() || policy_signed.is_some()),
        policy_unsigned: policy_unsigned.unwrap_or_default(),
        default_unsigned,
        minimum_unsigned,
        maximum_unsigned,
        policy_signed: policy_signed.unwrap_or_default(),
        default_signed,
        minimum_signed,
        maximum_signed,
        flags,
        output_unsigned: 0,
        output_signed: 0,
    };
    let status = unsafe {
        prodex_runtime_config_scalar_v1(
            RUNTIME_CONFIG_SCALAR_ABI_VERSION,
            &mut record as *mut RuntimeConfigScalarRecordV1 as usize as u64,
            std::mem::size_of::<RuntimeConfigScalarRecordV1>() as i64,
        )
    };
    let status = match status {
        -3 => return Err(crate::MojoError::AbiMismatch),
        -2 => return Err(crate::MojoError::InvalidInput),
        status => RuntimeConfigScalarStatus::from_i64(status)?,
    };
    Ok((record.output_unsigned, record.output_signed, status))
}

fn runtime_config_scalar_flags(trim: bool, allow_zero: bool, clamp: bool) -> i64 {
    i64::from(trim) * RUNTIME_CONFIG_SCALAR_FLAG_TRIM
        | i64::from(allow_zero) * RUNTIME_CONFIG_SCALAR_FLAG_ALLOW_ZERO
        | i64::from(clamp) * RUNTIME_CONFIG_SCALAR_FLAG_CLAMP
}

/// Parse and bound a strict positive unsigned runtime setting in Mojo.
pub fn runtime_config_scalar_positive_u64(
    input: RuntimeConfigScalarInput<'_>,
    policy: Option<u64>,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<(u64, RuntimeConfigScalarStatus), crate::MojoError> {
    let (value, _, status) = runtime_config_scalar_call(
        RuntimeConfigScalarKind::StrictPositiveUnsigned,
        input,
        policy,
        default,
        minimum,
        maximum,
        None,
        0,
        0,
        i64::MAX,
        runtime_config_scalar_flags(false, false, true),
    )?;
    if value < minimum || value > maximum {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok((value, status))
}

/// Parse a strict unsigned runtime setting, preserving the target-width limit.
pub fn runtime_config_scalar_unsigned(
    input: RuntimeConfigScalarInput<'_>,
    policy: Option<u64>,
    default: u64,
    maximum: u64,
) -> Result<(u64, RuntimeConfigScalarStatus), crate::MojoError> {
    let (value, _, status) = runtime_config_scalar_call(
        RuntimeConfigScalarKind::StrictUnsigned,
        input,
        policy,
        default,
        0,
        maximum,
        None,
        0,
        0,
        i64::MAX,
        runtime_config_scalar_flags(false, true, false),
    )?;
    if value > maximum {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok((value, status))
}

/// Parse and bound a strict positive signed runtime setting in Mojo.
pub fn runtime_config_scalar_positive_i64(
    input: RuntimeConfigScalarInput<'_>,
    policy: Option<i64>,
    default: i64,
    minimum: i64,
    maximum: i64,
) -> Result<(i64, RuntimeConfigScalarStatus), crate::MojoError> {
    let (_, value, status) = runtime_config_scalar_call(
        RuntimeConfigScalarKind::StrictPositiveSigned,
        input,
        None,
        0,
        0,
        u64::MAX,
        policy,
        default,
        minimum,
        maximum,
        0,
    )?;
    if value < minimum || value > maximum {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok((value, status))
}

/// Parse a strict boolean runtime setting in Mojo.
pub fn runtime_config_scalar_bool(
    input: RuntimeConfigScalarInput<'_>,
    default: bool,
) -> Result<(bool, RuntimeConfigScalarStatus), crate::MojoError> {
    let (value, _, status) = runtime_config_scalar_call(
        RuntimeConfigScalarKind::StrictBool,
        input,
        None,
        u64::from(default),
        0,
        1,
        None,
        0,
        0,
        i64::MAX,
        0,
    )?;
    match value {
        0 => Ok((false, status)),
        1 => Ok((true, status)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

/// Parse a compatibility unsigned runtime setting in Mojo.
pub fn runtime_config_scalar_compat_u64(
    input: RuntimeConfigScalarInput<'_>,
    fallback: u64,
    trim: bool,
    allow_zero: bool,
    maximum: u64,
) -> Result<(u64, RuntimeConfigScalarStatus), crate::MojoError> {
    let (value, _, status) = runtime_config_scalar_call(
        RuntimeConfigScalarKind::CompatibilityUnsigned,
        input,
        None,
        fallback,
        0,
        maximum,
        None,
        0,
        0,
        i64::MAX,
        runtime_config_scalar_flags(trim, allow_zero, true),
    )?;
    if value > maximum {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok((value, status))
}

/// Parse a compatibility boolean flag in Mojo.
pub fn runtime_config_scalar_compat_flag(
    input: RuntimeConfigScalarInput<'_>,
) -> Result<bool, crate::MojoError> {
    let (value, _, _) = runtime_config_scalar_call(
        RuntimeConfigScalarKind::CompatibilityFlag,
        input,
        None,
        0,
        0,
        1,
        None,
        0,
        0,
        i64::MAX,
        RUNTIME_CONFIG_SCALAR_FLAG_TRIM,
    )?;
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

/// Parse Gemini's compatibility boolean, whose unknown values stay enabled.
pub fn runtime_config_scalar_compat_bool_default_true(
    input: RuntimeConfigScalarInput<'_>,
) -> Result<(bool, RuntimeConfigScalarStatus), crate::MojoError> {
    let (value, _, status) = runtime_config_scalar_call(
        RuntimeConfigScalarKind::CompatibilityBoolDefaultTrue,
        input,
        None,
        1,
        0,
        1,
        None,
        0,
        0,
        i64::MAX,
        RUNTIME_CONFIG_SCALAR_FLAG_TRIM,
    )?;
    match value {
        0 => Ok((false, status)),
        1 => Ok((true, status)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_scalar_abi_rejects_invalid_version_without_writes() {
        let value = b"17";
        let mut record = RuntimeConfigScalarRecordV1 {
            operation: RUNTIME_CONFIG_SCALAR_STRICT_POSITIVE_UNSIGNED,
            value_address: value.as_ptr() as usize as u64,
            value_length: value.len() as i64,
            value_present: 1,
            value_valid_utf8: 1,
            policy_present: 0,
            policy_unsigned: 0,
            default_unsigned: 3,
            minimum_unsigned: 1,
            maximum_unsigned: 64,
            policy_signed: 0,
            default_signed: 0,
            minimum_signed: 0,
            maximum_signed: i64::MAX,
            flags: RUNTIME_CONFIG_SCALAR_FLAG_CLAMP,
            output_unsigned: u64::MAX,
            output_signed: i64::MIN,
        };
        let status = unsafe {
            prodex_runtime_config_scalar_v1(
                RUNTIME_CONFIG_SCALAR_ABI_VERSION + 1,
                &mut record as *mut RuntimeConfigScalarRecordV1 as usize as u64,
                std::mem::size_of::<RuntimeConfigScalarRecordV1>() as i64,
            )
        };
        assert_eq!(status, -3);
        assert_eq!(record.output_unsigned, u64::MAX);
        assert_eq!(record.output_signed, i64::MIN);

        let status = unsafe {
            prodex_runtime_config_scalar_v1(
                RUNTIME_CONFIG_SCALAR_ABI_VERSION,
                &mut record as *mut RuntimeConfigScalarRecordV1 as usize as u64,
                std::mem::size_of::<RuntimeConfigScalarRecordV1>() as i64 - 8,
            )
        };
        assert_eq!(status, -2);
        assert_eq!(record.output_unsigned, u64::MAX);
        assert_eq!(record.output_signed, i64::MIN);

        record.value_address = 0;
        record.value_length = 1;
        let status = unsafe {
            prodex_runtime_config_scalar_v1(
                RUNTIME_CONFIG_SCALAR_ABI_VERSION,
                &mut record as *mut RuntimeConfigScalarRecordV1 as usize as u64,
                std::mem::size_of::<RuntimeConfigScalarRecordV1>() as i64,
            )
        };
        assert_eq!(status, -2);
        assert_eq!(record.output_unsigned, u64::MAX);
        assert_eq!(record.output_signed, i64::MIN);

        let invalid_utf8 = [0xff_u8];
        record.value_address = invalid_utf8.as_ptr() as usize as u64;
        record.value_length = invalid_utf8.len() as i64;
        record.value_valid_utf8 = 1;
        let status = unsafe {
            prodex_runtime_config_scalar_v1(
                RUNTIME_CONFIG_SCALAR_ABI_VERSION,
                &mut record as *mut RuntimeConfigScalarRecordV1 as usize as u64,
                std::mem::size_of::<RuntimeConfigScalarRecordV1>() as i64,
            )
        };
        assert_eq!(status, -2);
        assert_eq!(record.output_unsigned, u64::MAX);
        assert_eq!(record.output_signed, i64::MIN);
    }
}
