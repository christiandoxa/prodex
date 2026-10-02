#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GeminiStringView {
    ptr: u64,
    len: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GeminiToolCallIndexRecordAbi {
    index: u64,
    explicit_call_id: i64,
    done: i64,
    name_present: i64,
    name: GeminiStringView,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GeminiToolCallIndexBindingAbi {
    id: GeminiStringView,
    index: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GeminiToolResponseOrderRecordAbi {
    response_present: i64,
    id: GeminiStringView,
}

const _: () = {
    assert!(std::mem::size_of::<GeminiStringView>() == 16);
    assert!(std::mem::align_of::<GeminiStringView>() == 8);
    assert!(std::mem::offset_of!(GeminiStringView, ptr) == 0);
    assert!(std::mem::offset_of!(GeminiStringView, len) == 8);
    assert!(std::mem::size_of::<GeminiToolCallIndexRecordAbi>() == 48);
    assert!(std::mem::offset_of!(GeminiToolCallIndexRecordAbi, name) == 32);
    assert!(std::mem::size_of::<GeminiToolCallIndexBindingAbi>() == 24);
    assert!(std::mem::size_of::<GeminiToolResponseOrderRecordAbi>() == 24);
    assert!(std::mem::offset_of!(GeminiToolResponseOrderRecordAbi, id) == 8);
};

const GEMINI_TOOL_CALL_INDEX_ABI_VERSION: i64 = 1;
const GEMINI_TOOL_RESPONSE_ORDER_ABI_VERSION: i64 = 1;

/// Existing Gemini SSE tool-call state projected into the index-selection ABI.
#[derive(Debug, Clone, Copy)]
pub struct GeminiToolCallIndexRecord<'a> {
    pub index: usize,
    pub explicit_call_id: bool,
    pub done: bool,
    pub name: Option<&'a str>,
}

/// Existing explicit Gemini tool-call ID binding projected into the index ABI.
#[derive(Debug, Clone, Copy)]
pub struct GeminiToolCallIndexBinding<'a> {
    pub id: &'a str,
    pub index: usize,
}

unsafe extern "C" {
    fn prodex_provider_constraints_gemini_tool_call_index_v1(
        abi_version: i64,
        part_index: u64,
        explicit_call_id_present: i64,
        explicit_call_id: *const GeminiStringView,
        name: *const GeminiStringView,
        records: *const GeminiToolCallIndexRecordAbi,
        record_count: i64,
        bindings: *const GeminiToolCallIndexBindingAbi,
        binding_count: i64,
        output_index: *mut u64,
    ) -> i64;
    fn prodex_provider_constraints_gemini_tool_response_order_v1(
        abi_version: i64,
        call_ids: *const GeminiStringView,
        call_count: i64,
        parts: *const GeminiToolResponseOrderRecordAbi,
        part_count: i64,
        output_order: *mut u64,
        output_ranks: *mut u64,
    ) -> i64;
}

fn string_view(value: &str) -> GeminiStringView {
    GeminiStringView {
        ptr: value.as_ptr() as u64,
        len: value.len() as u64,
    }
}

/// Selects the existing Rust tool-call index without crossing JSON or stream events.
pub fn gemini_tool_call_index(
    part_index: usize,
    explicit_call_id: Option<&str>,
    name: &str,
    records: &[GeminiToolCallIndexRecord<'_>],
    bindings: &[GeminiToolCallIndexBinding<'_>],
) -> Result<usize, crate::MojoError> {
    let explicit_call_id_present = explicit_call_id.is_some();
    let records = records
        .iter()
        .map(|record| {
            Ok(GeminiToolCallIndexRecordAbi {
                index: u64::try_from(record.index).map_err(|_| crate::MojoError::InvalidInput)?,
                explicit_call_id: i64::from(record.explicit_call_id),
                done: i64::from(record.done),
                name_present: i64::from(record.name.is_some()),
                name: record
                    .name
                    .map(string_view)
                    .unwrap_or(GeminiStringView { ptr: 0, len: 0 }),
            })
        })
        .collect::<Result<Vec<_>, crate::MojoError>>()?;
    let bindings = bindings
        .iter()
        .map(|binding| {
            Ok(GeminiToolCallIndexBindingAbi {
                id: string_view(binding.id),
                index: u64::try_from(binding.index).map_err(|_| crate::MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, crate::MojoError>>()?;
    let part_index = u64::try_from(part_index).map_err(|_| crate::MojoError::InvalidInput)?;
    let explicit_call_id = explicit_call_id
        .map(string_view)
        .unwrap_or(GeminiStringView { ptr: 0, len: 0 });
    let name = string_view(name);
    let mut output_index = 0_u64;
    let status = unsafe {
        prodex_provider_constraints_gemini_tool_call_index_v1(
            GEMINI_TOOL_CALL_INDEX_ABI_VERSION,
            part_index,
            i64::from(explicit_call_id_present),
            &explicit_call_id,
            &name,
            records.as_ptr(),
            i64::try_from(records.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            bindings.as_ptr(),
            i64::try_from(bindings.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            &mut output_index,
        )
    };
    match status {
        0 => usize::try_from(output_index).map_err(|_| crate::MojoError::InvalidOutput),
        1 => Err(crate::MojoError::AbiMismatch),
        2 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

/// Returns a stable permutation for Gemini function-response parts.
///
/// `None` marks a non-response part. `Some("")` represents a response with no
/// string ID, matching the request-history compatibility behavior.
pub fn gemini_tool_response_part_order(
    call_ids: &[&str],
    response_ids: &[Option<&str>],
) -> Result<Vec<usize>, crate::MojoError> {
    let call_ids = call_ids
        .iter()
        .map(|id| string_view(id))
        .collect::<Vec<_>>();
    let parts = response_ids
        .iter()
        .map(|id| GeminiToolResponseOrderRecordAbi {
            response_present: i64::from(id.is_some()),
            id: id
                .map(string_view)
                .unwrap_or(GeminiStringView { ptr: 0, len: 0 }),
        })
        .collect::<Vec<_>>();
    let part_count = i64::try_from(parts.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    let mut output_order = vec![0_u64; parts.len()];
    let mut output_ranks = vec![0_u64; parts.len()];
    let status = unsafe {
        prodex_provider_constraints_gemini_tool_response_order_v1(
            GEMINI_TOOL_RESPONSE_ORDER_ABI_VERSION,
            call_ids.as_ptr(),
            i64::try_from(call_ids.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            parts.as_ptr(),
            part_count,
            output_order.as_mut_ptr(),
            output_ranks.as_mut_ptr(),
        )
    };
    match status {
        0 => {}
        1 => return Err(crate::MojoError::AbiMismatch),
        2 => return Err(crate::MojoError::InvalidInput),
        _ => return Err(crate::MojoError::InvalidOutput),
    }

    let mut seen = vec![false; parts.len()];
    output_order
        .into_iter()
        .map(|index| {
            let index = usize::try_from(index).map_err(|_| crate::MojoError::InvalidOutput)?;
            let Some(seen) = seen.get_mut(index) else {
                return Err(crate::MojoError::InvalidOutput);
            };
            if *seen {
                return Err(crate::MojoError::InvalidOutput);
            }
            *seen = true;
            Ok(index)
        })
        .collect()
}

#[cfg(all(test, feature = "mojo-provider-constraints"))]
mod tests {
    use super::gemini_tool_response_part_order;

    #[test]
    fn real_mojo_orders_duplicate_and_unmatched_ids_stably() {
        assert_eq!(
            gemini_tool_response_part_order(
                &["call-a", "call-b", "call-a"],
                &[
                    None,
                    Some("missing-a"),
                    Some("call-b"),
                    Some("call-a"),
                    Some("call-a"),
                    Some("missing-b"),
                    Some(""),
                ],
            )
            .unwrap(),
            [3, 4, 2, 1, 5, 6, 0]
        );
        assert_eq!(
            gemini_tool_response_part_order(&["", "call-a"], &[Some("call-a"), Some("")]).unwrap(),
            [1, 0]
        );
        assert_eq!(
            gemini_tool_response_part_order(&[], &[Some("call-a"), None, Some("call-b")]).unwrap(),
            [0, 1, 2]
        );
    }
}
