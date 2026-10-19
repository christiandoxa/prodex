use crate::MojoError;

const GEMINI_BRIDGE_REQUEST_ABI_VERSION: i64 = 1;
const GEMINI_BRIDGE_REQUEST_STATUS_INVALID: i64 = 1;
const GEMINI_BRIDGE_REQUEST_STATUS_CAPACITY: i64 = 2;
const GEMINI_BRIDGE_REQUEST_STATUS_ABI_MISMATCH: i64 = 3;
const GEMINI_BRIDGE_REQUEST_MAX_BYTES: usize = 4 * 1024 * 1024;
const GEMINI_SIGNATURE_PRESERVATION_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Deterministic request operations that remain after the Gemini content kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiBridgeRequestOperation {
    GenerateContentRequest = 1,
    GenerateContentBody = 2,
    GenerationConfig = 3,
    NativeProject = 4,
    RequestBodyWithoutTool = 5,
    SimpleRequest = 6,
    ValidateCandidateCount = 7,
    ToolConfig = 8,
    TextContents = 9,
    RawTranslatorRequest = 10,
    ValidateTranslatorRequest = 11,
    PreserveToolCallSignatures = 12,
    HardenToolCallThoughtSignatures = 13,
    ValidateToolArray = 14,
}

impl GeminiBridgeRequestOperation {
    const fn code(self) -> i64 {
        self as i64
    }
}

/// Borrowed JSON fragments for one bounded Gemini bridge request operation.
#[derive(Debug, Clone, Copy)]
pub struct GeminiBridgeRequestKernelInput<'a> {
    pub operation: GeminiBridgeRequestOperation,
    pub primary: Option<&'a [u8]>,
    pub secondary: Option<&'a [u8]>,
    pub tertiary: Option<&'a [u8]>,
    pub quaternary: Option<&'a [u8]>,
    pub quinary: Option<&'a [u8]>,
    pub senary: Option<&'a [u8]>,
    pub septenary: Option<&'a [u8]>,
    pub octonary: Option<&'a [u8]>,
    pub kind: i64,
}

impl<'a> GeminiBridgeRequestKernelInput<'a> {
    pub const fn new(operation: GeminiBridgeRequestOperation) -> Self {
        Self {
            operation,
            primary: None,
            secondary: None,
            tertiary: None,
            quaternary: None,
            quinary: None,
            senary: None,
            septenary: None,
            octonary: None,
            kind: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct GeminiBridgeRequestStringView {
    ptr: u64,
    len: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct GeminiBridgeRequestFfiInput {
    operation: i64,
    primary: GeminiBridgeRequestStringView,
    secondary: GeminiBridgeRequestStringView,
    tertiary: GeminiBridgeRequestStringView,
    quaternary: GeminiBridgeRequestStringView,
    quinary: GeminiBridgeRequestStringView,
    senary: GeminiBridgeRequestStringView,
    septenary: GeminiBridgeRequestStringView,
    octonary: GeminiBridgeRequestStringView,
    primary_present: i64,
    secondary_present: i64,
    tertiary_present: i64,
    quaternary_present: i64,
    quinary_present: i64,
    senary_present: i64,
    septenary_present: i64,
    octonary_present: i64,
    kind: i64,
}

const _: () = assert!(std::mem::size_of::<GeminiBridgeRequestStringView>() == 16);
const _: () = assert!(std::mem::size_of::<GeminiBridgeRequestFfiInput>() == 208);

unsafe extern "C" {
    fn prodex_gemini_builtin_tool_batch_v1(
        abi_version: i64,
        request_address: u64,
        request_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_gemini_bridge_request_kernel_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

fn pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

fn mutable_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}

fn view(value: Option<&[u8]>) -> GeminiBridgeRequestStringView {
    value
        .map(|value| GeminiBridgeRequestStringView {
            ptr: pointer_address(value.as_ptr()),
            len: value.len() as u64,
        })
        .unwrap_or_default()
}

fn output_capacity(input: &GeminiBridgeRequestKernelInput<'_>) -> Result<usize, MojoError> {
    let max_bytes = if matches!(
        input.operation,
        GeminiBridgeRequestOperation::PreserveToolCallSignatures
            | GeminiBridgeRequestOperation::HardenToolCallThoughtSignatures
    ) {
        GEMINI_SIGNATURE_PRESERVATION_MAX_BYTES
    } else {
        GEMINI_BRIDGE_REQUEST_MAX_BYTES
    };
    [
        input.primary,
        input.secondary,
        input.tertiary,
        input.quaternary,
        input.quinary,
        input.senary,
        input.septenary,
        input.octonary,
    ]
    .into_iter()
    .flatten()
    .try_fold(0_usize, |total, value| {
        if value.len() > max_bytes {
            return Err(MojoError::InvalidInput);
        }
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })
    .and_then(|total| {
        total
            .checked_mul(4)
            .and_then(|value| value.checked_add(4096))
            .ok_or(MojoError::InvalidInput)
    })
}

/// Compiled batch classifier for raw Gemini builtin tool entries.
/// The input is a Serde-normalized JSON object containing a tools array.
/// The caller only applies selected positions; Rust has no per-item
/// supported-tool predicate or SimpleRequest-based fallback.
pub fn gemini_builtin_tool_positions(
    request: &[u8],
    expected_count: usize,
) -> Result<Vec<usize>, MojoError> {
    let mut flags = vec![-1_i64; expected_count];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_gemini_builtin_tool_batch_v1(
            GEMINI_BRIDGE_REQUEST_ABI_VERSION,
            pointer_address(request.as_ptr()),
            i64::try_from(request.len()).map_err(|_| MojoError::InvalidInput)?,
            mutable_pointer_address(flags.as_mut_ptr()),
            i64::try_from(flags.len()).map_err(|_| MojoError::InvalidInput)?,
            mutable_pointer_address(&mut written),
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count != expected_count {
        return Err(MojoError::InvalidOutput);
    }
    flags
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| match selected {
            0 => None,
            1 => Some(Ok(index)),
            _ => Some(Err(MojoError::InvalidOutput)),
        })
        .collect()
}

/// Runs one bounded, caller-owned Gemini bridge request operation in Mojo.
pub fn gemini_bridge_request_kernel(
    input: GeminiBridgeRequestKernelInput<'_>,
) -> Result<Vec<u8>, MojoError> {
    let capacity = output_capacity(&input)?;
    let mut output = vec![0_u8; capacity];
    let ffi_input = GeminiBridgeRequestFfiInput {
        operation: input.operation.code(),
        primary: view(input.primary),
        secondary: view(input.secondary),
        tertiary: view(input.tertiary),
        quaternary: view(input.quaternary),
        quinary: view(input.quinary),
        senary: view(input.senary),
        septenary: view(input.septenary),
        octonary: view(input.octonary),
        primary_present: i64::from(input.primary.is_some()),
        secondary_present: i64::from(input.secondary.is_some()),
        tertiary_present: i64::from(input.tertiary.is_some()),
        quaternary_present: i64::from(input.quaternary.is_some()),
        quinary_present: i64::from(input.quinary.is_some()),
        senary_present: i64::from(input.senary.is_some()),
        septenary_present: i64::from(input.septenary.is_some()),
        octonary_present: i64::from(input.octonary.is_some()),
        kind: input.kind,
    };
    let mut written = 0_i64;
    let status = unsafe {
        prodex_gemini_bridge_request_kernel_v1(
            GEMINI_BRIDGE_REQUEST_ABI_VERSION,
            pointer_address(&ffi_input),
            mutable_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mutable_pointer_address(&mut written),
        )
    };
    match status {
        0 => {}
        GEMINI_BRIDGE_REQUEST_STATUS_INVALID => return Err(MojoError::InvalidInput),
        GEMINI_BRIDGE_REQUEST_STATUS_CAPACITY => return Err(MojoError::Capacity),
        GEMINI_BRIDGE_REQUEST_STATUS_ABI_MISMATCH => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}

#[cfg(test)]
mod gemini_builtin_batch_tests {
    use super::*;

    #[test]
    fn compiled_gemini_tool_array_validator_preserves_first_error_index_and_local_media() {
        let request = br#"{"tools":[
            {"type":"web_search"},
            {"type":"function","function":{"name":"","parameters":{}}},
            {"type":"function","function":{"name":"valid","parameters":{}}}
        ]}"#;
        let plan = gemini_bridge_request_kernel(GeminiBridgeRequestKernelInput {
            operation: GeminiBridgeRequestOperation::ValidateToolArray,
            primary: Some(request),
            ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::ValidateToolArray)
        })
        .unwrap();
        let text = std::str::from_utf8(&plan).unwrap();
        assert!(text.contains(r#""index":1"#), "{text}");
        assert!(text.contains(r#"tools[1].function.name"#), "{text}");

        let mixed_media = br#"{"tools":[
            {"type":"function","function":null,
             "attachment":{"type":"image_url","file_path":"/tmp/example.png"}},
            {"type":"function","function":{"name":"fine","parameters":{}}}
        ]}"#;
        let valid = gemini_bridge_request_kernel(GeminiBridgeRequestKernelInput {
            operation: GeminiBridgeRequestOperation::ValidateToolArray,
            primary: Some(mixed_media),
            ..GeminiBridgeRequestKernelInput::new(GeminiBridgeRequestOperation::ValidateToolArray)
        })
        .unwrap();
        assert!(std::str::from_utf8(&valid).unwrap().contains(r#""tag":0"#));
        assert_eq!(
            gemini_bridge_request_kernel(GeminiBridgeRequestKernelInput {
                operation: GeminiBridgeRequestOperation::ValidateToolArray,
                primary: Some(request),
                kind: 2,
                ..GeminiBridgeRequestKernelInput::new(
                    GeminiBridgeRequestOperation::ValidateToolArray
                )
            }),
            Err(MojoError::InvalidInput),
        );
    }

    #[test]
    fn compiled_gemini_tool_array_validator_skips_only_verified_bridge_positions() {
        let request = br#"{"tools":[
            {"type":"tool_search","parameters":{"type":"object"}},
            {"type":"function","function":{"name":"","parameters":{}}},
            {"type":"web_search"}
        ]}"#;
        let operation = GeminiBridgeRequestOperation::ValidateToolArray;
        let plan = |verified: Option<&[u8]>| {
            gemini_bridge_request_kernel(GeminiBridgeRequestKernelInput {
                operation,
                primary: Some(request),
                secondary: verified,
                ..GeminiBridgeRequestKernelInput::new(operation)
            })
        };
        let initial = String::from_utf8(plan(None).unwrap()).unwrap();
        assert!(initial.contains(r#""tag":16"#), "{initial}");
        assert!(initial.contains(r#""index":0"#), "{initial}");

        let after_bridge = String::from_utf8(plan(Some(br#""100""#)).unwrap()).unwrap();
        assert!(after_bridge.contains(r#""index":1"#), "{after_bridge}");
        assert!(
            after_bridge.contains("tools[1].function.name"),
            "{after_bridge}"
        );

        let skipped_all = String::from_utf8(plan(Some(br#""110""#)).unwrap()).unwrap();
        assert!(skipped_all.contains(r#""tag":0"#), "{skipped_all}");
        for malformed in [
            br#""10""#.as_slice(),
            br#""1000""#.as_slice(),
            br#""10X""#.as_slice(),
            b"true".as_slice(),
        ] {
            assert_eq!(plan(Some(malformed)), Err(MojoError::InvalidInput));
        }
    }

    #[test]
    fn compiled_gemini_builtin_batch_recognizes_aliases_and_sanitized_tool_set_in_one_call() {
        let body = br#"{"tools":[
            {"type":"web_search"},
            {"type":"function","function":{"name":"lookup"}},
            {"type":"computerUse","function":{"name":"ignored"}},
            {"type":"url_context"},
            {"type":"my_unknown_tool"},
            {"type":"web_search_preview_2099"},
            {"computerUse":{}},
            {"type":"web_fetch_preview_new"},
            {"type":"function","name":"x"},
            {"type":"code_interpreter"},
            {"type":"codeExecution"}
        ]}"#;
        assert_eq!(
            gemini_builtin_tool_positions(body, 11),
            Ok(vec![0, 2, 3, 5, 6, 7, 9, 10]),
        );
        assert_eq!(
            gemini_builtin_tool_positions(br#"{"tools":[]}"#, 0),
            Ok(vec![]),
        );
        assert_eq!(
            gemini_builtin_tool_positions(body, 10),
            Err(MojoError::Capacity),
        );
        assert_eq!(
            gemini_builtin_tool_positions(body, 12),
            Err(MojoError::InvalidOutput),
        );
        assert_eq!(
            gemini_builtin_tool_positions(
                br#"{"tools":[{"type":"function"},{"type":"function"}]}"#,
                2
            ),
            Ok(vec![]),
        );
    }

    #[test]
    fn compiled_gemini_builtin_batch_abi_rejects_bad_request_and_capacity_without_writes() {
        let request = br#"{"tools":[{"type":"web_search"}]}"#;
        let input = request.as_ptr() as usize as u64;
        let mut output = [71_i64; 2];
        let mut written = 83_i64;
        let dest = output.as_mut_ptr() as usize as u64;
        let count = (&mut written as *mut i64) as usize as u64;
        let call = |version, ptr, len, output, cap, count| unsafe {
            prodex_gemini_builtin_tool_batch_v1(version, ptr, len, output, cap, count)
        };
        assert_eq!(call(2, input, request.len() as i64, dest, 2, count), 4);
        assert_eq!(call(1, 0, request.len() as i64, dest, 2, count), 1);
        assert_eq!(call(1, input, -1, dest, 2, count), 1);
        assert_eq!(call(1, input, request.len() as i64, 0, 2, count), 1);
        assert_eq!(call(1, input, request.len() as i64, dest, -1, count), 1);
        assert_eq!(call(1, input, request.len() as i64, dest, 0, count), 3);
        assert_eq!(call(1, input, request.len() as i64, dest, 2, 0), 1);
        let invalid = br#"{"tools":["#;
        assert_eq!(
            call(
                1,
                invalid.as_ptr() as usize as u64,
                invalid.len() as i64,
                dest,
                2,
                count
            ),
            1
        );
        let malformed = br#"{"tools":"not an array"}"#;
        assert_eq!(
            call(
                1,
                malformed.as_ptr() as usize as u64,
                malformed.len() as i64,
                dest,
                2,
                count
            ),
            1
        );
        assert_eq!(output, [71; 2]);
        assert_eq!(written, 83);
    }
}
