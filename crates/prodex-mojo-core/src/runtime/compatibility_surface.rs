use crate::MojoError;

const COMPATIBILITY_SURFACE_ABI_VERSION: i64 = 1;
const COMPATIBILITY_SURFACE_MAX_TOOL_LABELS: usize = 1_024;

#[repr(C)]
#[derive(Clone, Copy)]
struct CompatibilityStringView {
    ptr: u64,
    len: u64,
}

const _: () = {
    assert!(std::mem::size_of::<CompatibilityStringView>() == 16);
    assert!(std::mem::align_of::<CompatibilityStringView>() == 8);
};

unsafe extern "C" {
    fn prodex_runtime_compatibility_surface_plan_v1(
        abi_version: i64,
        route_kind: i64,
        transport_websocket: i64,
        codex_headers: i64,
        subagent_header: i64,
        internal_origin: i64,
        explicit_stream: i64,
        previous_response_present: i64,
        turn_state_present: i64,
        session_present: i64,
        tools_present: i64,
        user_agent_address: u64,
        user_agent_length: i64,
        tool_views_address: u64,
        tool_count: i64,
        output_address: u64,
    ) -> i64;
}

pub fn compatibility_surface_plan(
    facts: [i64; 10],
    user_agent: &str,
    tool_labels: &[&str],
) -> Result<[i64; 7], MojoError> {
    if tool_labels.len() > COMPATIBILITY_SURFACE_MAX_TOOL_LABELS {
        return Err(MojoError::InvalidInput);
    }
    let tool_views = tool_labels
        .iter()
        .map(|value| CompatibilityStringView {
            ptr: value.as_ptr() as usize as u64,
            len: value.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut output = [0_i64; 7];
    let status = unsafe {
        prodex_runtime_compatibility_surface_plan_v1(
            COMPATIBILITY_SURFACE_ABI_VERSION,
            facts[0],
            facts[1],
            facts[2],
            facts[3],
            facts[4],
            facts[5],
            facts[6],
            facts[7],
            facts[8],
            facts[9],
            user_agent.as_ptr() as usize as u64,
            i64::try_from(user_agent.len()).map_err(|_| MojoError::InvalidInput)?,
            tool_views.as_ptr() as usize as u64,
            i64::try_from(tool_views.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}
