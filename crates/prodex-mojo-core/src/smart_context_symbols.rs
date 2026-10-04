use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 517;
const NONE: u64 = u64::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolLabel {
    Function,
    Test,
    Symbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolStyle {
    Brace,
    Python,
}

/// One Mojo-classified declaration. Rust owns source-range planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolClassification {
    pub line_index: usize,
    pub label: SymbolLabel,
    pub symbol: String,
    pub style: SymbolStyle,
}

unsafe extern "C" {
    fn prodex_smart_context_symbol_classify_v1(
        abi_version: i64,
        line_spans_address: u64,
        line_count: i64,
        start_index: i64,
        output_address: u64,
        output_capacity: i64,
        metadata_address: u64,
    ) -> i64;
}

fn classification(
    lines: &[&str],
    spans: &[u64],
    start_index: usize,
) -> Result<Option<SymbolClassification>, MojoError> {
    let mut output = vec![0_u8; MAX_NAME_BYTES];
    let mut metadata = [NONE; 4];
    let status = unsafe {
        prodex_smart_context_symbol_classify_v1(
            ABI_VERSION,
            spans.as_ptr() as usize as u64,
            i64::try_from(lines.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(start_index).map_err(|_| MojoError::InvalidInput)?,
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
    if metadata[0] == NONE {
        return (metadata[1..] == [0, 0, 0])
            .then_some(None)
            .ok_or(MojoError::InvalidOutput);
    }
    let line_index = usize::try_from(metadata[0]).map_err(|_| MojoError::InvalidOutput)?;
    if line_index < start_index || line_index >= lines.len() {
        return Err(MojoError::InvalidOutput);
    }
    let label = match metadata[1] {
        0 => SymbolLabel::Function,
        1 => SymbolLabel::Test,
        2 => SymbolLabel::Symbol,
        _ => return Err(MojoError::InvalidOutput),
    };
    let style = match metadata[2] {
        0 => SymbolStyle::Brace,
        1 => SymbolStyle::Python,
        _ => return Err(MojoError::InvalidOutput),
    };
    let written = usize::try_from(metadata[3]).map_err(|_| MojoError::InvalidOutput)?;
    if written == 0 || written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    let symbol =
        String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)?;
    Ok(Some(SymbolClassification {
        line_index,
        label,
        symbol,
        style,
    }))
}

/// Visit declarations classified by the required Mojo ABI.
///
/// The callback returns `false` to stop scanning. No Rust classifier or
/// feature-off implementation is used.
pub fn visit_classifications(
    lines: &[&str],
    mut visit: impl FnMut(SymbolClassification) -> bool,
) -> Result<(), MojoError> {
    if lines.is_empty() {
        return Ok(());
    }
    let input_bytes = lines
        .iter()
        .try_fold(0usize, |total, line| total.checked_add(line.len()))
        .and_then(|total| total.checked_add(lines.len().saturating_sub(1)))
        .ok_or(MojoError::InvalidInput)?;
    if input_bytes > MAX_INPUT_BYTES || lines.len() > MAX_INPUT_BYTES.saturating_add(1) {
        return Err(MojoError::InvalidInput);
    }

    let mut spans = Vec::with_capacity(lines.len().checked_mul(2).ok_or(MojoError::InvalidInput)?);
    for line in lines {
        spans.push(u64::try_from(line.as_ptr() as usize).map_err(|_| MojoError::InvalidInput)?);
        spans.push(u64::try_from(line.len()).map_err(|_| MojoError::InvalidInput)?);
    }
    let mut start_index = 0;
    while let Some(classification) = classification(lines, &spans, start_index)? {
        start_index = classification.line_index + 1;
        if !visit(classification) {
            break;
        }
    }
    Ok(())
}
