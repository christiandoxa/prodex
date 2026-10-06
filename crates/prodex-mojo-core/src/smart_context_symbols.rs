use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_SYMBOL_RANGES: usize = 256;
const MAX_EXCERPT_BYTES: usize = 16 * 1024;
const RECORD_WIDTH: usize = 7;
const NONE: u64 = u64::MAX;

unsafe extern "C" {
    fn prodex_smart_context_symbol_index_v1(
        abi_version: i64,
        text_address: u64,
        text_length: i64,
        line_spans_address: u64,
        line_count: i64,
        max_ranges: i64,
        max_excerpt_bytes: i64,
        output_address: u64,
        output_capacity: i64,
        metadata_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolLabel {
    Function,
    Test,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRangePlan {
    pub start_line: usize,
    pub end_line: usize,
    pub declaration_line: usize,
    pub label: SymbolLabel,
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolIndexPlan {
    pub complete: bool,
    pub ranges: Vec<SymbolRangePlan>,
}

fn validate_symbol_index_input(
    text: &str,
    max_ranges: usize,
    max_excerpt_bytes: usize,
) -> Result<(), MojoError> {
    if text.len() > MAX_INPUT_BYTES
        || max_ranges > MAX_SYMBOL_RANGES
        || max_excerpt_bytes > MAX_EXCERPT_BYTES
    {
        return Err(MojoError::InvalidInput);
    }
    Ok(())
}

fn symbol_line_spans(text: &str, lines: &[&str]) -> Result<Vec<u64>, MojoError> {
    let mut line_spans = Vec::with_capacity(lines.len() * 2);
    let text_address = text.as_ptr() as usize;
    for line in lines {
        let start = (line.as_ptr() as usize)
            .checked_sub(text_address)
            .ok_or(MojoError::InvalidInput)?;
        let end = start
            .checked_add(line.len())
            .ok_or(MojoError::InvalidInput)?;
        if text.get(start..end) != Some(*line) {
            return Err(MojoError::InvalidInput);
        }
        line_spans.push(u64::try_from(start).map_err(|_| MojoError::InvalidInput)?);
        line_spans.push(u64::try_from(end).map_err(|_| MojoError::InvalidInput)?);
    }
    Ok(line_spans)
}

fn call_symbol_index(
    text: &str,
    lines: &[&str],
    line_spans: &[u64],
    max_ranges: usize,
    max_excerpt_bytes: usize,
) -> Result<(Vec<u64>, [u64; 2]), MojoError> {
    let output_capacity = max_ranges
        .checked_mul(RECORD_WIDTH)
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u64; output_capacity];
    let mut metadata = [NONE; 2];
    let status = unsafe {
        prodex_smart_context_symbol_index_v1(
            ABI_VERSION,
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            line_spans.as_ptr() as usize as u64,
            i64::try_from(lines.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_ranges).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_excerpt_bytes).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            metadata.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok((output, metadata)),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_symbol_index_metadata(
    metadata: [u64; 2],
    max_ranges: usize,
    output_len: usize,
) -> Result<(usize, bool), MojoError> {
    let count = usize::try_from(metadata[0]).map_err(|_| MojoError::InvalidOutput)?;
    let complete = match metadata[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    if count > max_ranges
        || count
            .checked_mul(RECORD_WIDTH)
            .is_none_or(|written| written > output_len)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok((count, complete))
}

fn decode_symbol_label(value: u64) -> Result<SymbolLabel, MojoError> {
    match value {
        0 => Ok(SymbolLabel::Function),
        1 => Ok(SymbolLabel::Test),
        2 => Ok(SymbolLabel::Symbol),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_named_symbol(text: &str, record: &[u64; RECORD_WIDTH]) -> Result<String, MojoError> {
    let start = usize::try_from(record[3]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(record[4]).map_err(|_| MojoError::InvalidOutput)?;
    let name = text.get(start..end).ok_or(MojoError::InvalidOutput)?;
    if name.is_empty() || name.len() > 512 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(name.to_owned())
}

fn decode_symbol_name(
    text: &str,
    record: &[u64; RECORD_WIDTH],
    label: SymbolLabel,
) -> Result<String, MojoError> {
    match record[6] {
        0 => decode_named_symbol(text, record),
        1 if label == SymbolLabel::Symbol => {
            Ok(format!("impl {}", decode_named_symbol(text, record)?))
        }
        2 if label == SymbolLabel::Test && record[3] == NONE && record[4] == NONE => {
            Ok("test".to_owned())
        }
        3 if label == SymbolLabel::Test && record[3] == NONE && record[4] == NONE => {
            Ok("it".to_owned())
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_symbol_range(
    text: &str,
    line_count: usize,
    record: &[u64; RECORD_WIDTH],
) -> Result<SymbolRangePlan, MojoError> {
    let start_line = usize::try_from(record[0]).map_err(|_| MojoError::InvalidOutput)?;
    let end_line = usize::try_from(record[1]).map_err(|_| MojoError::InvalidOutput)?;
    let declaration_line = usize::try_from(record[2]).map_err(|_| MojoError::InvalidOutput)?;
    if start_line == 0
        || start_line > declaration_line
        || declaration_line > end_line
        || end_line > line_count
    {
        return Err(MojoError::InvalidOutput);
    }
    let label = decode_symbol_label(record[5])?;
    Ok(SymbolRangePlan {
        start_line,
        end_line,
        declaration_line,
        label,
        symbol: decode_symbol_name(text, record, label)?,
    })
}

fn decode_symbol_ranges(
    text: &str,
    line_count: usize,
    output: &[u64],
    count: usize,
) -> Result<Vec<SymbolRangePlan>, MojoError> {
    let mut ranges = Vec::with_capacity(count);
    for record in output.as_chunks::<RECORD_WIDTH>().0.iter().take(count) {
        ranges.push(decode_symbol_range(text, line_count, record)?);
    }
    Ok(ranges)
}

/// Plans bounded source declaration ranges through the versioned Mojo kernel.
///
/// `text` is the borrowed UTF-8 source. Rust prepares line byte spans and
/// materializes the validated Mojo plan; Mojo owns declaration recognition,
/// range boundaries, completeness, and capacity decisions.
pub fn index(
    text: &str,
    max_ranges: usize,
    max_excerpt_bytes: usize,
) -> Result<SymbolIndexPlan, MojoError> {
    validate_symbol_index_input(text, max_ranges, max_excerpt_bytes)?;
    let lines = text.lines().collect::<Vec<_>>();
    let line_spans = symbol_line_spans(text, &lines)?;
    let (output, metadata) =
        call_symbol_index(text, &lines, &line_spans, max_ranges, max_excerpt_bytes)?;
    let (count, complete) = decode_symbol_index_metadata(metadata, max_ranges, output.len())?;
    Ok(SymbolIndexPlan {
        complete,
        ranges: decode_symbol_ranges(text, lines.len(), &output, count)?,
    })
}
