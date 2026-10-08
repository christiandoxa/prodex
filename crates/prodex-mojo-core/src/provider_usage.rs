use crate::MojoError;

const ABI_VERSION: i64 = 1;
const ESTIMATE_JSON_VALUE: i64 = 1;
const ESTIMATE_TEXT: i64 = 2;
const ESTIMATE_REQUEST: i64 = 3;

/// Borrowed JSON facts marshalled to the provider usage estimator.
#[derive(Clone, Copy, Debug)]
pub struct ProviderUsageJsonNode<'a> {
    pub kind: i64,
    pub first_child: Option<usize>,
    pub next_sibling: Option<usize>,
    pub parent: Option<usize>,
    pub key: &'a str,
    pub text: &'a str,
    pub raw_start: usize,
    pub raw_length: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UsageStringView {
    address: u64,
    length: u64,
}

impl From<&str> for UsageStringView {
    fn from(value: &str) -> Self {
        Self {
            address: value.as_ptr() as usize as u64,
            length: value.len() as u64,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UsageJsonNode {
    kind: i64,
    first_child: i64,
    next_sibling: i64,
    parent: i64,
    key: UsageStringView,
    text: UsageStringView,
    raw_start: i64,
    raw_length: i64,
}

const _: () = {
    assert!(std::mem::size_of::<UsageStringView>() == 16);
    assert!(std::mem::size_of::<UsageJsonNode>() == 80);
    assert!(std::mem::align_of::<UsageJsonNode>() == 8);
};

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
    fn prodex_provider_usage_merge_latest_present_v1(
        abi_version: i64,
        previous_input_present: i64,
        previous_input_tokens: u64,
        incoming_input_present: i64,
        incoming_input_tokens: u64,
        previous_output_present: i64,
        previous_output_tokens: u64,
        incoming_output_present: i64,
        incoming_output_tokens: u64,
        previous_total_present: i64,
        previous_total_tokens: u64,
        incoming_total_present: i64,
        incoming_total_tokens: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_provider_usage_estimate_v1(
        abi_version: i64,
        operation: i64,
        nodes_address: u64,
        nodes_count: i64,
        raw_address: u64,
        raw_length: i64,
        text_address: u64,
        text_length: i64,
        body_empty: i64,
        output_address: u64,
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

fn signed(value: usize) -> Result<i64, MojoError> {
    i64::try_from(value).map_err(|_| MojoError::InvalidInput)
}

fn optional_node_index(value: Option<usize>, count: usize) -> Result<i64, MojoError> {
    match value {
        None => Ok(-1),
        Some(value) if value < count => signed(value),
        _ => Err(MojoError::InvalidInput),
    }
}

fn estimate_nodes(
    nodes: &[ProviderUsageJsonNode<'_>],
    raw: &[u8],
) -> Result<Vec<UsageJsonNode>, MojoError> {
    if nodes.is_empty() || nodes.len() > i64::MAX as usize / 80 {
        return Err(MojoError::InvalidInput);
    }
    nodes
        .iter()
        .map(|node| {
            let end = node
                .raw_start
                .checked_add(node.raw_length)
                .ok_or(MojoError::InvalidInput)?;
            raw.get(node.raw_start..end)
                .ok_or(MojoError::InvalidInput)?;
            Ok(UsageJsonNode {
                kind: node.kind,
                first_child: optional_node_index(node.first_child, nodes.len())?,
                next_sibling: optional_node_index(node.next_sibling, nodes.len())?,
                parent: optional_node_index(node.parent, nodes.len())?,
                key: node.key.into(),
                text: node.text.into(),
                raw_start: signed(node.raw_start)?,
                raw_length: signed(node.raw_length)?,
            })
        })
        .collect()
}

fn estimate(
    operation: i64,
    tree: Option<(&[ProviderUsageJsonNode<'_>], &[u8])>,
    text: &str,
    body_empty: bool,
) -> Result<Option<u64>, MojoError> {
    let (nodes, raw) = tree.unwrap_or((&[], &[]));
    let ffi_nodes = if nodes.is_empty() {
        Vec::new()
    } else {
        estimate_nodes(nodes, raw)?
    };
    let mut output = [0_u64; 2];
    status(unsafe {
        prodex_provider_usage_estimate_v1(
            ABI_VERSION,
            operation,
            if ffi_nodes.is_empty() {
                0
            } else {
                ffi_nodes.as_ptr() as usize as u64
            },
            signed(ffi_nodes.len())?,
            if raw.is_empty() {
                0
            } else {
                raw.as_ptr() as usize as u64
            },
            signed(raw.len())?,
            if text.is_empty() {
                0
            } else {
                text.as_ptr() as usize as u64
            },
            signed(text.len())?,
            i64::from(body_empty),
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    output_option(output[0], output[1])
}

/// Estimate request input tokens from a Serde-built JSON tree or its text form.
pub fn estimate_request_tokens(
    tree: Option<(&[ProviderUsageJsonNode<'_>], &[u8])>,
    text: &str,
    body_empty: bool,
) -> Result<u64, MojoError> {
    estimate(ESTIMATE_REQUEST, tree, text, body_empty)?.ok_or(MojoError::InvalidOutput)
}

/// Estimate selected text fields from a Serde-built JSON value.
pub fn estimate_json_tokens(
    nodes: &[ProviderUsageJsonNode<'_>],
    raw: &[u8],
) -> Result<Option<u64>, MojoError> {
    estimate(ESTIMATE_JSON_VALUE, Some((nodes, raw)), "", false)
}

/// Estimate tokens in already-decoded text.
pub fn estimate_text_tokens(text: &str) -> Result<u64, MojoError> {
    estimate(ESTIMATE_TEXT, None, text, false)?.ok_or(MojoError::InvalidOutput)
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

pub fn merge_latest_present(
    previous: ProviderUsagePlan,
    incoming: ProviderUsagePlan,
) -> Result<ProviderUsagePlan, MojoError> {
    let mut output = [0_u64; 6];
    status(unsafe {
        prodex_provider_usage_merge_latest_present_v1(
            ABI_VERSION,
            option_flag(previous.input_tokens),
            option_value(previous.input_tokens),
            option_flag(incoming.input_tokens),
            option_value(incoming.input_tokens),
            option_flag(previous.output_tokens),
            option_value(previous.output_tokens),
            option_flag(incoming.output_tokens),
            option_value(incoming.output_tokens),
            option_flag(previous.total_tokens),
            option_value(previous.total_tokens),
            option_flag(incoming.total_tokens),
            option_value(incoming.total_tokens),
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(ProviderUsagePlan {
        input_tokens: output_option(output[0], output[1])?,
        output_tokens: output_option(output[2], output[3])?,
        total_tokens: output_option(output[4], output[5])?,
    })
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

    #[test]
    fn provider_usage_latest_present_merge_updates_across_events() {
        let first = merge_latest_present(
            ProviderUsagePlan::default(),
            ProviderUsagePlan {
                input_tokens: Some(10),
                output_tokens: Some(20),
                total_tokens: Some(30),
            },
        )
        .unwrap();
        let second = merge_latest_present(
            first,
            ProviderUsagePlan {
                input_tokens: Some(11),
                output_tokens: None,
                total_tokens: None,
            },
        )
        .unwrap();
        let third = merge_latest_present(
            second,
            ProviderUsagePlan {
                input_tokens: None,
                output_tokens: Some(22),
                total_tokens: Some(33),
            },
        )
        .unwrap();

        assert_eq!(
            third,
            ProviderUsagePlan {
                input_tokens: Some(11),
                output_tokens: Some(22),
                total_tokens: Some(33),
            }
        );
    }
}
