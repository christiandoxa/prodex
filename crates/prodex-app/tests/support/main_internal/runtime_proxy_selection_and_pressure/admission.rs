use super::*;

#[path = "admission/cli_mount.rs"]
mod cli_mount;
#[path = "admission/compact.rs"]
mod compact;
#[path = "admission/continuation_store.rs"]
mod continuation_store;
#[path = "admission/doctor_summary.rs"]
mod doctor_summary;
#[path = "admission/guards.rs"]
mod guards;
#[path = "admission/helpers.rs"]
mod helpers;
#[path = "admission/local_capacity.rs"]
mod local_capacity;
#[path = "admission/rotation_matrix.rs"]
mod rotation_matrix;
#[path = "admission/pre_send.rs"]
mod pre_send;
#[path = "admission/pressure_budget.rs"]
mod pressure_budget;
#[path = "admission/quota_fallback.rs"]
mod quota_fallback;
#[path = "admission/previous_response.rs"]
mod previous_response;
#[path = "admission/response_affinity.rs"]
mod response_affinity;
#[path = "admission/sse_tap.rs"]
mod sse_tap;
#[path = "admission/turn_state.rs"]
mod turn_state;
