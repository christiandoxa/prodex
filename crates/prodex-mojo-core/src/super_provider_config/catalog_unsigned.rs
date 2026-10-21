//! Checked scalar adapter for provider catalog limits; Mojo owns parsing and bounds.
use crate::MojoError;

/// Original catalog validation outcome, including the integer source diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogUnsignedPlan {
    Value(u64),
    Empty,
    Whitespace,
    InvalidUnsignedInteger { reason: &'static str },
    TooSmall,
}

unsafe extern "C" {
    fn prodex_provider_catalog_uint_v1(version: i64, address: u64, length: i64, output: u64)
    -> i64;
}

/// Validates an unsigned catalog setting greater than one, without trimming.
pub fn catalog_unsigned_value(value: &str) -> Result<CatalogUnsignedPlan, MojoError> {
    let mut output = [0_u64; 2];
    let status = unsafe {
        prodex_provider_catalog_uint_v1(
            1,
            value.as_ptr() as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
        )
    };
    match status {
        0 if output[0] > 1 && output[1] == 0 => Ok(CatalogUnsignedPlan::Value(output[0])),
        11 => Ok(CatalogUnsignedPlan::Empty),
        12 => Ok(CatalogUnsignedPlan::Whitespace),
        13 => match output[1] {
            1 => Ok(CatalogUnsignedPlan::InvalidUnsignedInteger {
                reason: "invalid digit found in string",
            }),
            2 => Ok(CatalogUnsignedPlan::InvalidUnsignedInteger {
                reason: "number too large to fit in target type",
            }),
            _ => Err(MojoError::InvalidOutput),
        },
        14 => Ok(CatalogUnsignedPlan::TooSmall),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "catalog_unsigned_tests.rs"]
mod tests;
