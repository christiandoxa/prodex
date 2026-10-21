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
    fn prodex_provider_usage_sse_scan_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        measuring: i64,
        written_address: u64,
    ) -> i64;
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

/// A borrowed SSE data segment or end-of-frame marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderUsageSseFrameRecord {
    Data { start: usize, end: usize },
    End,
}

/// The compiled provider-usage kernel owns SSE line classification,
/// CRLF normalization, optional leading-space stripping and blank-event
/// boundaries. This adapter validates spans; the caller keeps Serde parsing.
pub fn sse_frame_scan(text: &str) -> Result<Vec<ProviderUsageSseFrameRecord>, MojoError> {
    let length = signed(text.len())?;
    let mut record_count = -1_i64;
    let input_address = text.as_ptr() as usize as u64;
    let written_address = (&mut record_count as *mut i64) as usize as u64;
    let invoke = |output: u64, capacity: i64, measuring: i64| unsafe {
        prodex_provider_usage_sse_scan_v1(
            ABI_VERSION,
            input_address,
            length,
            output,
            capacity,
            measuring,
            written_address,
        )
    };
    status(invoke(0, 0, 1))?;
    let count = usize::try_from(record_count).map_err(|_| MojoError::InvalidOutput)?;
    if count > text.len().saturating_mul(2).saturating_add(1) {
        return Err(MojoError::InvalidOutput);
    }
    let total_words = count.checked_mul(3).ok_or(MojoError::Capacity)?;
    let mut words = Vec::<i64>::new();
    words
        .try_reserve_exact(total_words)
        .map_err(|_| MojoError::Capacity)?;
    words.resize(total_words, -7);
    record_count = -1;
    status(invoke(
        words.as_mut_ptr() as usize as u64,
        signed(count)?,
        0,
    ))?;
    if usize::try_from(record_count).map_err(|_| MojoError::InvalidOutput)? != count {
        return Err(MojoError::InvalidOutput);
    }
    let mut result = Vec::with_capacity(count);
    let mut pending = false;
    for words in words.as_chunks::<3>().0 {
        match words {
            [1, start, end] => {
                let start = usize::try_from(*start).map_err(|_| MojoError::InvalidOutput)?;
                let end = usize::try_from(*end).map_err(|_| MojoError::InvalidOutput)?;
                if start > end || text.get(start..end).is_none() {
                    return Err(MojoError::InvalidOutput);
                }
                result.push(ProviderUsageSseFrameRecord::Data { start, end });
                pending = true;
            }
            [2, 0, 0] if pending => {
                result.push(ProviderUsageSseFrameRecord::End);
                pending = false;
            }
            _ => return Err(MojoError::InvalidOutput),
        }
    }
    if pending {
        return Err(MojoError::InvalidOutput);
    }
    Ok(result)
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
    fn compiled_provider_usage_sse_frames_preserve_crlf_blank_and_multiline_data() {
        use ProviderUsageSseFrameRecord::{Data, End};
        let text = "event:ignored\r\ndata: {\"usage\":{\r\ndata:  \"input_tokens\":12}}\r\n\r\ndata: [DONE]\n\ndata:\ndata: 雪 \r\r\n\n";
        let records = sse_frame_scan(text).unwrap();
        let frames = [
            ["{\"usage\":{", " \"input_tokens\":12}}"].as_slice(),
            ["[DONE]"].as_slice(),
            ["", "雪 "].as_slice(),
        ];
        let mut found = Vec::<Vec<&str>>::new();
        let mut current = Vec::new();
        for record in records {
            match record {
                Data { start, end } => current.push(&text[start..end]),
                End => found.push(std::mem::take(&mut current)),
            }
        }
        assert!(current.is_empty());
        assert_eq!(found, frames);
        assert_eq!(sse_frame_scan(""), Ok(Vec::new()));
        assert_eq!(sse_frame_scan("\n\n"), Ok(Vec::new()));
        assert_eq!(
            sse_frame_scan("data: unclosed"),
            Ok(vec![Data { start: 6, end: 14 }, End]),
        );
        assert_eq!(
            sse_frame_scan("data: \r\r"),
            Ok(vec![Data { start: 6, end: 6 }, End]),
        );
    }

    #[test]
    fn compiled_provider_usage_sse_scan_rejects_invalid_abi_and_capacity_before_output_writes() {
        let text = "data: first\n\ndata: second\n\n";
        let mut buffer = [79_i64; 18];
        let mut written = 83_i64;
        let output_address = buffer.as_mut_ptr() as usize as u64;
        let written_address = (&mut written as *mut i64) as usize as u64;
        let input_address = text.as_ptr() as usize as u64;
        for (version, input, input_len, output, capacity, measuring, written_ptr, expected) in [
            (
                2,
                input_address,
                text.len() as i64,
                0,
                0,
                1,
                written_address,
                4,
            ),
            (1, 0, text.len() as i64, 0, 0, 1, written_address, 1),
            (1, input_address, -1, 0, 0, 1, written_address, 1),
            (
                1,
                input_address,
                text.len() as i64,
                0,
                0,
                2,
                written_address,
                1,
            ),
            (1, input_address, text.len() as i64, 0, 0, 1, 0, 1),
            (
                1,
                input_address,
                text.len() as i64,
                output_address,
                3,
                0,
                written_address,
                3,
            ),
        ] {
            assert_eq!(
                unsafe {
                    prodex_provider_usage_sse_scan_v1(
                        version,
                        input,
                        input_len,
                        output,
                        capacity,
                        measuring,
                        written_ptr,
                    )
                },
                expected,
            );
            assert_eq!(buffer, [79; 18]);
            assert_eq!(written, 83);
        }
        let bad = [0xff_u8, 0xfe_u8];
        assert_eq!(
            unsafe {
                prodex_provider_usage_sse_scan_v1(
                    1,
                    bad.as_ptr() as usize as u64,
                    2,
                    0,
                    0,
                    1,
                    written_address,
                )
            },
            1,
        );
        assert_eq!(written, 83);
    }

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
