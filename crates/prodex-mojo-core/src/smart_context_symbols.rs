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
    if text.len() > MAX_INPUT_BYTES
        || max_ranges > MAX_SYMBOL_RANGES
        || max_excerpt_bytes > MAX_EXCERPT_BYTES
    {
        return Err(MojoError::InvalidInput);
    }

    let lines = text.lines().collect::<Vec<_>>();
    let mut line_spans = Vec::with_capacity(lines.len() * 2);
    let text_address = text.as_ptr() as usize;
    for line in &lines {
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
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }

    let count = usize::try_from(metadata[0]).map_err(|_| MojoError::InvalidOutput)?;
    let complete = match metadata[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    if count > max_ranges
        || count
            .checked_mul(RECORD_WIDTH)
            .is_none_or(|written| written > output.len())
    {
        return Err(MojoError::InvalidOutput);
    }

    let mut ranges = Vec::with_capacity(count);
    for record in output.as_chunks::<RECORD_WIDTH>().0.iter().take(count) {
        let start_line = usize::try_from(record[0]).map_err(|_| MojoError::InvalidOutput)?;
        let end_line = usize::try_from(record[1]).map_err(|_| MojoError::InvalidOutput)?;
        let declaration_line = usize::try_from(record[2]).map_err(|_| MojoError::InvalidOutput)?;
        if start_line == 0
            || start_line > declaration_line
            || declaration_line > end_line
            || end_line > lines.len()
        {
            return Err(MojoError::InvalidOutput);
        }
        let label = match record[5] {
            0 => SymbolLabel::Function,
            1 => SymbolLabel::Test,
            2 => SymbolLabel::Symbol,
            _ => return Err(MojoError::InvalidOutput),
        };
        let symbol = match record[6] {
            0 | 1 => {
                let start = usize::try_from(record[3]).map_err(|_| MojoError::InvalidOutput)?;
                let end = usize::try_from(record[4]).map_err(|_| MojoError::InvalidOutput)?;
                let name = text.get(start..end).ok_or(MojoError::InvalidOutput)?;
                if name.is_empty() || name.len() > 512 {
                    return Err(MojoError::InvalidOutput);
                }
                if record[6] == 1 {
                    if label != SymbolLabel::Symbol {
                        return Err(MojoError::InvalidOutput);
                    }
                    format!("impl {name}")
                } else {
                    name.to_owned()
                }
            }
            2 if label == SymbolLabel::Test && record[3] == NONE && record[4] == NONE => {
                "test".to_owned()
            }
            3 if label == SymbolLabel::Test && record[3] == NONE && record[4] == NONE => {
                "it".to_owned()
            }
            _ => return Err(MojoError::InvalidOutput),
        };
        ranges.push(SymbolRangePlan {
            start_line,
            end_line,
            declaration_line,
            label,
            symbol,
        });
    }

    Ok(SymbolIndexPlan { complete, ranges })
}
