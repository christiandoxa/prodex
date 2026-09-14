use super::{
    CONTEXT_GIT_SEARCH_MAX_BYTES, CONTEXT_TEXT_ABI_VERSION, ProdexStringView, text_abi_is_ready,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandOutputLineClassification {
    pub flags: i64,
    pub noisy_label: i64,
    pub diagnostic_label: i64,
}

const _: () = assert!(
    std::mem::size_of::<CommandOutputLineClassification>() == 3 * std::mem::size_of::<i64>()
);

unsafe extern "C" {
    fn prodex_context_classify_command_output_line_v1(
        abi_version: i64,
        line: *const ProdexStringView,
        result: *mut CommandOutputLineClassification,
    ) -> i64;
    fn prodex_context_analyze_command_output_v1(
        abi_version: i64,
        lines: *const ProdexStringView,
        line_count: i64,
        output: *mut i64,
        output_count: i64,
    ) -> i64;
    fn prodex_context_select_critical_lines_v1(
        abi_version: i64,
        lines: *const ProdexStringView,
        normalized_keys: *const ProdexStringView,
        counts: *const i64,
        line_count: i64,
        budget: i64,
        output: *mut i64,
        output_count: *mut i64,
    ) -> i64;
}

const OUTPUT_ANALYSIS_WIDTH: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandOutputAnalysis {
    pub diagnostic_immediate: bool,
    pub diagnostic: [usize; 5],
    pub non_empty: usize,
    pub success_signals: usize,
    pub key_lines: usize,
    pub success_failure: bool,
}

pub fn analyze_command_output(lines: &[&str]) -> Result<CommandOutputAnalysis, crate::MojoError> {
    if lines.len() > 65_536 || !text_abi_is_ready() {
        return Err(crate::MojoError::InvalidInput);
    }
    let views = lines
        .iter()
        .map(|line| ProdexStringView {
            ptr: line.as_ptr(),
            len: line.len(),
        })
        .collect::<Vec<_>>();
    let mut output = [0_i64; OUTPUT_ANALYSIS_WIDTH];
    let status = unsafe {
        prodex_context_analyze_command_output_v1(
            CONTEXT_TEXT_ABI_VERSION,
            views.as_ptr(),
            i64::try_from(views.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr(),
            OUTPUT_ANALYSIS_WIDTH as i64,
        )
    };
    if status != 0
        || output.iter().any(|value| *value < 0)
        || !matches!(output[0], 0 | 1)
        || !matches!(output[9], 0 | 1)
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    let values = output.map(|value| usize::try_from(value).expect("validated Mojo count"));
    Ok(CommandOutputAnalysis {
        diagnostic_immediate: values[0] == 1,
        diagnostic: values[1..6].try_into().expect("fixed analysis width"),
        non_empty: values[6],
        success_signals: values[7],
        key_lines: values[8],
        success_failure: values[9] == 1,
    })
}

pub fn select_critical_lines(
    lines: &[&str],
    normalized_keys: &[&str],
    counts: &[[usize; 7]],
    budget: usize,
) -> Result<Vec<usize>, crate::MojoError> {
    if lines.len() != counts.len()
        || lines.len() != normalized_keys.len()
        || lines.len() > 65_536
        || budget == 0
        || !text_abi_is_ready()
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let views = lines
        .iter()
        .map(|line| ProdexStringView {
            ptr: line.as_ptr(),
            len: line.len(),
        })
        .collect::<Vec<_>>();
    let key_views = normalized_keys
        .iter()
        .map(|key| ProdexStringView {
            ptr: key.as_ptr(),
            len: key.len(),
        })
        .collect::<Vec<_>>();
    let counts = counts
        .iter()
        .flatten()
        .map(|value| i64::try_from(*value).map_err(|_| crate::MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = vec![0_i64; lines.len().max(1)];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_context_select_critical_lines_v1(
            CONTEXT_TEXT_ABI_VERSION,
            views.as_ptr(),
            key_views.as_ptr(),
            counts.as_ptr(),
            i64::try_from(lines.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(budget).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr(),
            &mut output_count,
        )
    };
    if status != 0 || output_count < 0 || output_count as usize > lines.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    output[..output_count as usize]
        .iter()
        .map(|index| {
            usize::try_from(*index)
                .ok()
                .filter(|index| *index < lines.len())
                .ok_or(crate::MojoError::InvalidOutput)
        })
        .collect()
}

pub fn classify_command_output_line(
    line: &str,
) -> Result<CommandOutputLineClassification, crate::MojoError> {
    if line.len() > CONTEXT_GIT_SEARCH_MAX_BYTES {
        return Err(crate::MojoError::InvalidInput);
    }
    if !text_abi_is_ready() {
        return Err(crate::MojoError::AbiMismatch);
    }
    let view = ProdexStringView {
        ptr: line.as_ptr(),
        len: line.len(),
    };
    let mut result = CommandOutputLineClassification::default();
    let status = unsafe {
        prodex_context_classify_command_output_line_v1(CONTEXT_TEXT_ABI_VERSION, &view, &mut result)
    };
    match status {
        0 if result.flags >= 0
            && result.noisy_label >= 0
            && result.diagnostic_label >= 0
            && result.noisy_label <= 100
            && result.diagnostic_label <= 100 =>
        {
            Ok(result)
        }
        4 => Err(crate::MojoError::AbiMismatch),
        1 | 2 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
