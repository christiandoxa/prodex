use crate::MojoError;

const APP_SERVER_BROKER_ABI_VERSION: i64 = 1;
const APP_SERVER_BROKER_METHOD_MAX_BYTES: usize = 1024 * 1024;
const APP_SERVER_BROKER_WIRE_OUTPUT_WIDTH: usize = 3;
const APP_SERVER_BROKER_METHOD_OUTPUT_WIDTH: usize = 3;
const APP_SERVER_BROKER_AFFINITY_OUTPUT_WIDTH: usize = 9;

const FRAME_KIND_INVALID: i64 = 0;
const FRAME_KIND_RESPONSE: i64 = 4;

const METHOD_KIND_ABSENT: i64 = 0;
const METHOD_KIND_OTHER: i64 = 2;

const REASON_NONE: i64 = 0;
const REASON_MISSING_METHOD_AND_RESPONSE_PAYLOAD: i64 = 17;

const SEQUENCE_EVENT_REQUEST: i64 = 1;
const SEQUENCE_EVENT_RESPONSE: i64 = 2;
const SEQUENCE_EVENT_LIFECYCLE: i64 = 3;
const SEQUENCE_REASON_REQUEST_MISSING_ID: i64 = 20;
const SEQUENCE_REASON_DUPLICATE_TURN_COMPLETED: i64 = 36;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct AppServerStringView {
    ptr: u64,
    len: u64,
}

const _: () = {
    assert!(std::mem::size_of::<AppServerStringView>() == 16);
    assert!(std::mem::align_of::<AppServerStringView>() == 8);
    assert!(std::mem::offset_of!(AppServerStringView, ptr) == 0);
    assert!(std::mem::offset_of!(AppServerStringView, len) == 8);
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireInput {
    pub jsonrpc_state: i64,
    pub id_kind: i64,
    pub params_kind: i64,
    pub error_kind: i64,
    pub error_code_kind: i64,
    pub error_message_kind: i64,
    pub method_kind: i64,
    pub has_result: i64,
    pub has_error: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WirePlan {
    pub valid_jsonrpc: bool,
    pub frame_kind: i64,
    pub invalid_reason: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodPlan {
    pub method_kind: i64,
    pub lifecycle_stage: i64,
    pub lifecycle_schema: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffinityPlan {
    pub key_count: usize,
    pub key_kinds: [i64; 3],
    pub decision: i64,
    pub mode: i64,
    pub routing_hint: i64,
    pub commit_boundary: i64,
    pub rotation_window: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct ValidationInput<'a> {
    pub response: bool,
    pub stage: &'a str,
    pub thread_id_present: bool,
    pub thread_object_id_present: bool,
    pub thread_status: Option<&'a str>,
    pub thread_active_flags_valid: bool,
    pub thread_object_context: bool,
    pub response_thread_context: bool,
    pub response_thread_context_valid: bool,
    pub response_thread_object_context: bool,
    pub turn_input: bool,
    pub turn_id_present: bool,
    pub turn_status: Option<&'a str>,
    pub turn_items: bool,
}

struct SequenceInput<'a> {
    event_kind: i64,
    stage: Option<&'a str>,
    id_present: bool,
    thread_id_present: bool,
    pending_request_present: bool,
    duplicate_pending_request: bool,
    started_turn_present: bool,
    completed_turn_present: bool,
    active_turn_present: bool,
    active_turn_matches: bool,
}

unsafe extern "C" {
    fn prodex_mojo_app_server_broker_wire_v1(
        abi_version: i64,
        jsonrpc_state: i64,
        id_kind: i64,
        params_kind: i64,
        error_kind: i64,
        error_code_kind: i64,
        error_message_kind: i64,
        method_kind: i64,
        method: u64,
        has_result: i64,
        has_error: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_app_server_broker_method_v1(
        abi_version: i64,
        frame_kind: i64,
        method_kind: i64,
        method: u64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_app_server_broker_response_schema_v1(
        abi_version: i64,
        request_stage: u64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_app_server_broker_affinity_v1(
        abi_version: i64,
        frame_kind: i64,
        lifecycle_stage: i64,
        method: u64,
        session_present: i64,
        thread_present: i64,
        turn_present: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_app_server_broker_validation_v1(
        abi_version: i64,
        response: i64,
        stage: u64,
        thread_id_present: i64,
        thread_object_id_present: i64,
        thread_status: u64,
        thread_active_flags_valid: i64,
        thread_object_context: i64,
        response_thread_context: i64,
        response_thread_context_valid: i64,
        response_thread_object_context: i64,
        turn_input: i64,
        turn_id_present: i64,
        turn_status: u64,
        turn_items: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_app_server_broker_sequence_v1(
        abi_version: i64,
        event_kind: i64,
        stage: u64,
        id_present: i64,
        thread_id_present: i64,
        pending_request_present: i64,
        duplicate_pending_request: i64,
        started_turn_present: i64,
        completed_turn_present: i64,
        active_turn_present: i64,
        active_turn_matches: i64,
        output: u64,
    ) -> i64;
}

#[inline]
fn pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

#[inline]
fn mutable_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}

#[inline]
fn flag(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn string_view(value: Option<&str>) -> AppServerStringView {
    AppServerStringView {
        ptr: value.map_or(0, |value| value.as_ptr() as usize as u64),
        len: value.map_or(0, |value| value.len() as u64),
    }
}

fn status_error(status: i64) -> MojoError {
    match status {
        1 | 2 => MojoError::InvalidInput,
        4 => MojoError::AbiMismatch,
        _ => MojoError::InvalidOutput,
    }
}

pub fn classify_wire(input: WireInput, method: Option<&str>) -> Result<WirePlan, MojoError> {
    if !(0..=2).contains(&input.jsonrpc_state)
        || !(0..=2).contains(&input.id_kind)
        || !(0..=2).contains(&input.params_kind)
        || !(0..=2).contains(&input.error_kind)
        || !(0..=2).contains(&input.error_code_kind)
        || !(0..=2).contains(&input.error_message_kind)
        || !(0..=2).contains(&input.method_kind)
        || !(0..=1).contains(&input.has_result)
        || !(0..=1).contains(&input.has_error)
        || method.is_some_and(|value| value.len() > APP_SERVER_BROKER_METHOD_MAX_BYTES)
    {
        return Err(MojoError::InvalidInput);
    }
    let method_view = string_view(method);
    let method_address = method.map_or(0, |_| pointer_address(&method_view));
    let mut output = [0_i64; APP_SERVER_BROKER_WIRE_OUTPUT_WIDTH];
    let status = unsafe {
        prodex_mojo_app_server_broker_wire_v1(
            APP_SERVER_BROKER_ABI_VERSION,
            input.jsonrpc_state,
            input.id_kind,
            input.params_kind,
            input.error_kind,
            input.error_code_kind,
            input.error_message_kind,
            input.method_kind,
            method_address,
            input.has_result,
            input.has_error,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    if !matches!(output[0], 0 | 1)
        || !(FRAME_KIND_INVALID..=FRAME_KIND_RESPONSE).contains(&output[1])
        || !(REASON_NONE..=REASON_MISSING_METHOD_AND_RESPONSE_PAYLOAD).contains(&output[2])
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(WirePlan {
        valid_jsonrpc: output[0] == 1,
        frame_kind: output[1],
        invalid_reason: output[2],
    })
}

pub fn normalize_method(method: Option<&str>, frame_kind: i64) -> Result<MethodPlan, MojoError> {
    if !(FRAME_KIND_INVALID..=FRAME_KIND_RESPONSE).contains(&frame_kind)
        || method.is_some_and(|value| value.len() > APP_SERVER_BROKER_METHOD_MAX_BYTES)
    {
        return Err(MojoError::InvalidInput);
    }
    let method_kind = if method.is_some() { 1 } else { 0 };
    let method_view = string_view(method);
    let method_address = method.map_or(0, |_| pointer_address(&method_view));
    let mut output = [0_i64; APP_SERVER_BROKER_METHOD_OUTPUT_WIDTH];
    let status = unsafe {
        prodex_mojo_app_server_broker_method_v1(
            APP_SERVER_BROKER_ABI_VERSION,
            frame_kind,
            method_kind,
            method_address,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    if !(METHOD_KIND_ABSENT..=METHOD_KIND_OTHER).contains(&output[0])
        || !(0..=14).contains(&output[1])
        || !(0..=8).contains(&output[2])
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(MethodPlan {
        method_kind: output[0],
        lifecycle_stage: output[1],
        lifecycle_schema: output[2],
    })
}

pub fn response_schema(request_stage: &str) -> Result<i64, MojoError> {
    if request_stage.len() > APP_SERVER_BROKER_METHOD_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let stage_view = string_view(Some(request_stage));
    let stage_address = pointer_address(&stage_view);
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_app_server_broker_response_schema_v1(
            APP_SERVER_BROKER_ABI_VERSION,
            stage_address,
            mutable_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    if !(0..=5).contains(&output) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(output)
}

pub fn plan_affinity(
    frame_kind: i64,
    lifecycle_stage: i64,
    method: Option<&str>,
    session_present: bool,
    thread_present: bool,
    turn_present: bool,
) -> Result<AffinityPlan, MojoError> {
    if !(FRAME_KIND_INVALID..=FRAME_KIND_RESPONSE).contains(&frame_kind)
        || !(0..=14).contains(&lifecycle_stage)
        || method.is_some_and(|value| value.len() > APP_SERVER_BROKER_METHOD_MAX_BYTES)
    {
        return Err(MojoError::InvalidInput);
    }
    let method_view = string_view(method);
    let method_address = method.map_or(0, |_| pointer_address(&method_view));
    let mut output = [0_i64; APP_SERVER_BROKER_AFFINITY_OUTPUT_WIDTH];
    let status = unsafe {
        prodex_mojo_app_server_broker_affinity_v1(
            APP_SERVER_BROKER_ABI_VERSION,
            frame_kind,
            lifecycle_stage,
            method_address,
            flag(session_present),
            flag(thread_present),
            flag(turn_present),
            mutable_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    let key_count = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if key_count > 3
        || output[1..4].iter().any(|kind| !(0..=3).contains(kind))
        || !(0..=3).contains(&output[4])
        || !(0..=3).contains(&output[5])
        || !(0..=3).contains(&output[6])
        || !(0..=1).contains(&output[7])
        || !(0..=1).contains(&output[8])
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(AffinityPlan {
        key_count,
        key_kinds: [output[1], output[2], output[3]],
        decision: output[4],
        mode: output[5],
        routing_hint: output[6],
        commit_boundary: output[7],
        rotation_window: output[8],
    })
}

pub fn lifecycle_validation_reason(input: ValidationInput<'_>) -> Result<Option<i64>, MojoError> {
    if input.stage.len() > APP_SERVER_BROKER_METHOD_MAX_BYTES
        || input
            .thread_status
            .is_some_and(|value| value.len() > APP_SERVER_BROKER_METHOD_MAX_BYTES)
        || input
            .turn_status
            .is_some_and(|value| value.len() > APP_SERVER_BROKER_METHOD_MAX_BYTES)
    {
        return Err(MojoError::InvalidInput);
    }
    let stage_view = string_view(Some(input.stage));
    let thread_status_view = string_view(input.thread_status);
    let turn_status_view = string_view(input.turn_status);
    let stage_address = pointer_address(&stage_view);
    let thread_status_address = if input.thread_status.is_some() {
        pointer_address(&thread_status_view)
    } else {
        0
    };
    let turn_status_address = if input.turn_status.is_some() {
        pointer_address(&turn_status_view)
    } else {
        0
    };
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_app_server_broker_validation_v1(
            APP_SERVER_BROKER_ABI_VERSION,
            flag(input.response),
            stage_address,
            flag(input.thread_id_present),
            flag(input.thread_object_id_present),
            thread_status_address,
            flag(input.thread_active_flags_valid),
            flag(input.thread_object_context),
            flag(input.response_thread_context),
            flag(input.response_thread_context_valid),
            flag(input.response_thread_object_context),
            flag(input.turn_input),
            flag(input.turn_id_present),
            turn_status_address,
            flag(input.turn_items),
            mutable_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    if !(-1..=19).contains(&output) {
        return Err(MojoError::InvalidOutput);
    }
    Ok((output >= 1).then_some(output))
}

fn sequence_reason(input: SequenceInput<'_>) -> Result<Option<i64>, MojoError> {
    let SequenceInput {
        event_kind,
        stage,
        id_present,
        thread_id_present,
        pending_request_present,
        duplicate_pending_request,
        started_turn_present,
        completed_turn_present,
        active_turn_present,
        active_turn_matches,
    } = input;
    if !(SEQUENCE_EVENT_REQUEST..=SEQUENCE_EVENT_LIFECYCLE).contains(&event_kind)
        || stage.is_some_and(|value| value.len() > APP_SERVER_BROKER_METHOD_MAX_BYTES)
    {
        return Err(MojoError::InvalidInput);
    }
    let stage_view = string_view(stage);
    let stage_address = stage.map_or(0, |_| pointer_address(&stage_view));
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_app_server_broker_sequence_v1(
            APP_SERVER_BROKER_ABI_VERSION,
            event_kind,
            stage_address,
            flag(id_present),
            flag(thread_id_present),
            flag(pending_request_present),
            flag(duplicate_pending_request),
            flag(started_turn_present),
            flag(completed_turn_present),
            flag(active_turn_present),
            flag(active_turn_matches),
            mutable_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    if output != 0
        && !(SEQUENCE_REASON_REQUEST_MISSING_ID..=SEQUENCE_REASON_DUPLICATE_TURN_COMPLETED)
            .contains(&output)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok((output != 0).then_some(output))
}

pub fn request_sequence_reason(
    id_present: bool,
    duplicate_pending_request: bool,
) -> Result<Option<i64>, MojoError> {
    sequence_reason(SequenceInput {
        event_kind: SEQUENCE_EVENT_REQUEST,
        stage: None,
        id_present,
        thread_id_present: false,
        pending_request_present: false,
        duplicate_pending_request,
        started_turn_present: false,
        completed_turn_present: false,
        active_turn_present: false,
        active_turn_matches: false,
    })
}

pub fn response_sequence_reason(
    id_present: bool,
    pending_request_present: bool,
) -> Result<Option<i64>, MojoError> {
    sequence_reason(SequenceInput {
        event_kind: SEQUENCE_EVENT_RESPONSE,
        stage: None,
        id_present,
        thread_id_present: false,
        pending_request_present,
        duplicate_pending_request: false,
        started_turn_present: false,
        completed_turn_present: false,
        active_turn_present: false,
        active_turn_matches: false,
    })
}

pub fn lifecycle_sequence_reason(
    stage: &str,
    turn_id_present: bool,
    thread_id_present: bool,
    completed_turn_present: bool,
    active_turn_present: bool,
    active_turn_matches: bool,
    started_turn_present: bool,
) -> Result<Option<i64>, MojoError> {
    sequence_reason(SequenceInput {
        event_kind: SEQUENCE_EVENT_LIFECYCLE,
        stage: Some(stage),
        id_present: turn_id_present,
        thread_id_present,
        pending_request_present: false,
        duplicate_pending_request: false,
        started_turn_present,
        completed_turn_present,
        active_turn_present,
        active_turn_matches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mojo_plans_wire_lifecycle_affinity_and_validation() {
        let wire = classify_wire(
            WireInput {
                jsonrpc_state: 1,
                id_kind: 1,
                params_kind: 1,
                error_kind: 0,
                error_code_kind: 0,
                error_message_kind: 0,
                method_kind: 1,
                has_result: 0,
                has_error: 0,
            },
            Some("thread/start"),
        )
        .expect("Mojo wire planner should accept a request");
        assert_eq!(
            wire,
            WirePlan {
                valid_jsonrpc: true,
                frame_kind: 2,
                invalid_reason: 0
            }
        );

        let method = normalize_method(Some(" thread/start "), wire.frame_kind)
            .expect("Mojo method planner should normalize a request");
        assert_eq!(method.method_kind, 1);
        assert_eq!(method.lifecycle_stage, 3);
        assert_eq!(method.lifecycle_schema, 1);

        let affinity = plan_affinity(
            wire.frame_kind,
            method.lifecycle_stage,
            Some("thread/start"),
            true,
            true,
            false,
        )
        .expect("Mojo affinity planner should accept a request");
        assert_eq!(affinity.key_count, 2);
        assert_eq!(affinity.key_kinds, [2, 1, 0]);
        assert_eq!(affinity.decision, 2);
        assert_eq!(affinity.rotation_window, 1);

        assert_eq!(
            lifecycle_validation_reason(ValidationInput {
                response: false,
                stage: "turn_start_request",
                thread_id_present: false,
                thread_object_id_present: false,
                thread_status: None,
                thread_active_flags_valid: true,
                thread_object_context: false,
                response_thread_context: false,
                response_thread_context_valid: false,
                response_thread_object_context: false,
                turn_input: false,
                turn_id_present: false,
                turn_status: None,
                turn_items: false,
            })
            .expect("Mojo validation planner should return a reason"),
            Some(1)
        );
        assert_eq!(
            lifecycle_validation_reason(ValidationInput {
                response: false,
                stage: "turn_started_notification",
                thread_id_present: false,
                thread_object_id_present: false,
                thread_status: None,
                thread_active_flags_valid: true,
                thread_object_context: false,
                response_thread_context: false,
                response_thread_context_valid: false,
                response_thread_object_context: false,
                turn_input: false,
                turn_id_present: true,
                turn_status: Some("inProgress"),
                turn_items: true,
            })
            .expect("Mojo validation planner should accept a valid turn notification"),
            None
        );
    }

    #[test]
    fn mojo_sequence_reason_matrix_matches_protocol_contract() {
        assert_eq!(request_sequence_reason(false, false).unwrap(), Some(20));
        assert_eq!(request_sequence_reason(true, true).unwrap(), Some(23));
        assert_eq!(request_sequence_reason(true, false).unwrap(), None);
        assert_eq!(response_sequence_reason(false, false).unwrap(), Some(21));
        assert_eq!(response_sequence_reason(true, false).unwrap(), Some(22));
        assert_eq!(response_sequence_reason(true, true).unwrap(), None);

        let stage = "turn_started_notification";
        assert_eq!(
            lifecycle_sequence_reason(stage, false, true, false, false, false, false).unwrap(),
            Some(24)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, false, false, false, false, false).unwrap(),
            Some(25)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, true, true, false, false, false).unwrap(),
            Some(31)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, true, false, true, false, false).unwrap(),
            Some(32)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, true, false, false, false, true).unwrap(),
            Some(35)
        );

        let stage = "turn_completed_notification";
        assert_eq!(
            lifecycle_sequence_reason(stage, false, true, false, false, false, true).unwrap(),
            Some(26)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, false, false, false, false, true).unwrap(),
            Some(27)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, true, false, false, false, false).unwrap(),
            Some(30)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, true, false, true, false, true).unwrap(),
            Some(33)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, true, true, false, false, true).unwrap(),
            Some(36)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, true, false, false, false, true).unwrap(),
            None
        );

        let stage = "turn_interrupt_request";
        assert_eq!(
            lifecycle_sequence_reason(stage, false, true, false, false, false, false).unwrap(),
            Some(28)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, false, false, false, false, false).unwrap(),
            Some(29)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, true, false, true, false, false).unwrap(),
            Some(34)
        );
        assert_eq!(
            lifecycle_sequence_reason(stage, true, true, false, true, true, false).unwrap(),
            None
        );
    }
}
