use super::{
    MojoError, MojoIssue, RICH_ABI_VERSION, RichStringView, ensure_rich_abi,
    mojo_mut_pointer_address, mojo_pointer_address, view,
};

/// Deterministic Gemini request-configuration transformations.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiConfigKernelOperation {
    ModelUsesThinkingLevel = 1,
    ThinkingConfig = 2,
    TextFormat = 3,
    ResponseFormat = 4,
    ContinuationMetadata = 5,
    ToolCallSignatures = 6,
    ValidateCandidateCount = 7,
}

/// Borrowed inputs for one bounded Gemini configuration transformation.
#[derive(Debug, Clone, Copy)]
pub struct GeminiConfigKernelInput<'a> {
    pub operation: GeminiConfigKernelOperation,
    pub primary: Option<&'a str>,
    pub secondary: Option<&'a str>,
    pub tertiary: Option<&'a str>,
    pub quaternary: Option<&'a str>,
    pub number: Option<u64>,
}

impl<'a> GeminiConfigKernelInput<'a> {
    pub const fn new(operation: GeminiConfigKernelOperation) -> Self {
        Self {
            operation,
            primary: None,
            secondary: None,
            tertiary: None,
            quaternary: None,
            number: None,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GeminiConfigKernelFfiInput {
    operation: i64,
    number: u64,
    number_present: i64,
    primary_present: i64,
    secondary_present: i64,
    tertiary_present: i64,
    quaternary_present: i64,
    primary: RichStringView,
    secondary: RichStringView,
    tertiary: RichStringView,
    quaternary: RichStringView,
}

const _: () = assert!(std::mem::size_of::<GeminiConfigKernelFfiInput>() == 120);

unsafe extern "C" {
    fn prodex_mojo_gemini_config_kernel_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

const GEMINI_CONFIG_KERNEL_MAX_BYTES: usize = 4 * 1024 * 1024;

fn kernel_view(value: Option<&str>) -> RichStringView {
    value.map(view).unwrap_or_default()
}

fn operation_code(operation: GeminiConfigKernelOperation) -> i64 {
    match operation {
        GeminiConfigKernelOperation::ModelUsesThinkingLevel => 1,
        GeminiConfigKernelOperation::ThinkingConfig => 2,
        GeminiConfigKernelOperation::TextFormat => 3,
        GeminiConfigKernelOperation::ResponseFormat => 4,
        GeminiConfigKernelOperation::ContinuationMetadata => 5,
        GeminiConfigKernelOperation::ToolCallSignatures => 6,
        GeminiConfigKernelOperation::ValidateCandidateCount => 7,
    }
}

fn input_bytes(input: &GeminiConfigKernelInput<'_>) -> Result<usize, MojoError> {
    [
        input.primary,
        input.secondary,
        input.tertiary,
        input.quaternary,
    ]
    .iter()
    .flatten()
    .try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })
}

/// Runs one bounded Gemini configuration transformation in compiled Mojo.
pub fn gemini_config_kernel(input: GeminiConfigKernelInput<'_>) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    let input_bytes = input_bytes(&input)?;
    if input_bytes > GEMINI_CONFIG_KERNEL_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let capacity = input_bytes
        .checked_mul(8)
        .and_then(|value| value.checked_add(4096))
        .ok_or(MojoError::InvalidInput)?;
    let ffi_input = GeminiConfigKernelFfiInput {
        operation: operation_code(input.operation),
        number: input.number.unwrap_or_default(),
        number_present: i64::from(input.number.is_some()),
        primary_present: i64::from(input.primary.is_some()),
        secondary_present: i64::from(input.secondary.is_some()),
        tertiary_present: i64::from(input.tertiary.is_some()),
        quaternary_present: i64::from(input.quaternary.is_some()),
        primary: kernel_view(input.primary),
        secondary: kernel_view(input.secondary),
        tertiary: kernel_view(input.tertiary),
        quaternary: kernel_view(input.quaternary),
    };
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_gemini_config_kernel_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(match status {
            2 => MojoError::Structured(MojoIssue {
                domain: 9,
                kind: 1,
                field: 0,
                object_index: -1,
                byte_offset: 0,
                byte_length: 1,
                expected: 0,
            }),
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            1 => MojoError::InvalidInput,
            _ => MojoError::InvalidOutput,
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}
