use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderUsagePlan {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

unsafe extern "C" {
    fn prodex_provider_usage_extract_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_provider_usage_cost_v1(
        abi_version: i64,
        input_present: i64,
        input_tokens: u64,
        input_rate_present: i64,
        input_rate: u64,
        output_present: i64,
        output_tokens: u64,
        output_rate_present: i64,
        output_rate: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_provider_usage_merged_total_v1(
        abi_version: i64,
        total_present: i64,
        total_tokens: u64,
        input_present: i64,
        input_tokens: u64,
        output_present: i64,
        output_tokens: u64,
        result_address: u64,
    ) -> i64;
}

fn status(code: i64) -> Result<(), MojoError> {
    match code {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn option_flag(value: Option<u64>) -> i64 {
    i64::from(value.is_some())
}

fn option_value(value: Option<u64>) -> u64 {
    value.unwrap_or_default()
}

fn output_option(flag: u64, value: u64) -> Result<Option<u64>, MojoError> {
    match flag {
        0 => Ok(None),
        1 => Ok(Some(value)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn extract_json(source: &str) -> Result<ProviderUsagePlan, MojoError> {
    let mut output = [0_u64; 6];
    status(unsafe {
        prodex_provider_usage_extract_v1(
            ABI_VERSION,
            source.as_ptr() as usize as u64,
            i64::try_from(source.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(ProviderUsagePlan {
        input_tokens: output_option(output[0], output[1])?,
        output_tokens: output_option(output[2], output[3])?,
        total_tokens: output_option(output[4], output[5])?,
    })
}

pub fn calculate_cost(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    input_rate: Option<u64>,
    output_rate: Option<u64>,
) -> Result<Option<u64>, MojoError> {
    let mut result = [0_u64; 2];
    status(unsafe {
        prodex_provider_usage_cost_v1(
            ABI_VERSION,
            option_flag(input_tokens),
            option_value(input_tokens),
            option_flag(input_rate),
            option_value(input_rate),
            option_flag(output_tokens),
            option_value(output_tokens),
            option_flag(output_rate),
            option_value(output_rate),
            result.as_mut_ptr() as usize as u64,
        )
    })?;
    output_option(result[0], result[1])
}

pub fn merged_total(
    total_tokens: Option<u64>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
) -> Result<Option<u64>, MojoError> {
    let mut result = [0_u64; 2];
    status(unsafe {
        prodex_provider_usage_merged_total_v1(
            ABI_VERSION,
            option_flag(total_tokens),
            option_value(total_tokens),
            option_flag(input_tokens),
            option_value(input_tokens),
            option_flag(output_tokens),
            option_value(output_tokens),
            result.as_mut_ptr() as usize as u64,
        )
    })?;
    output_option(result[0], result[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_usage_kernel_smoke() {
        assert_eq!(
            extract_json(r#"{"usage":{"input_tokens":10,"output_tokens":20,"total_tokens":30}}"#)
                .unwrap(),
            ProviderUsagePlan {
                input_tokens: Some(10),
                output_tokens: Some(20),
                total_tokens: Some(30),
            }
        );
        assert_eq!(
            extract_json(
                r#"{"usageMetadata":{"promptTokenCount":11,"candidatesTokenCount":22,"totalTokenCount":33}}"#
            )
            .unwrap(),
            ProviderUsagePlan {
                input_tokens: Some(11),
                output_tokens: Some(22),
                total_tokens: Some(33),
            }
        );
        assert_eq!(
            calculate_cost(Some(1_000), Some(2_000), Some(1_000_000), Some(2_000_000)).unwrap(),
            Some(5_000)
        );
        assert_eq!(
            merged_total(None, Some(u64::MAX), Some(1)).unwrap(),
            Some(u64::MAX)
        );
    }
}
