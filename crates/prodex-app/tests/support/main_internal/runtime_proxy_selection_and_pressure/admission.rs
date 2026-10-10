use super::*;

#[path = "admission/compact.rs"]
mod compact;
#[path = "admission/continuation_store.rs"]
mod continuation_store;
#[path = "admission/doctor_summary.rs"]
mod doctor_summary;
#[path = "admission/guards.rs"]
mod guards;
#[path = "admission/lock_order.rs"]
mod lock_order;
#[path = "admission/hard_affinity_replay.rs"]
mod hard_affinity_replay;
#[path = "admission/helpers.rs"]
mod helpers;
#[path = "admission/local_capacity.rs"]
mod local_capacity;
#[path = "admission/pre_send.rs"]
mod pre_send;
#[path = "admission/pressure_budget.rs"]
mod pressure_budget;
#[path = "admission/previous_response.rs"]
mod previous_response;
#[path = "admission/quota_fallback.rs"]
mod quota_fallback;
#[path = "admission/response_affinity.rs"]
mod response_affinity;
#[path = "admission/responses_overload_recovery.rs"]
mod responses_overload_recovery;
#[path = "admission/responses_missing_content_type.rs"]
mod responses_missing_content_type;
#[path = "admission/retired_spark.rs"]
mod retired_spark;
#[path = "admission/rotation_matrix.rs"]
mod rotation_matrix;
#[path = "admission/sse_tap.rs"]
mod sse_tap;
#[path = "admission/standard_retry_recovery.rs"]
mod standard_retry_recovery;
#[path = "admission/standard_session_recovery.rs"]
mod standard_session_recovery;
#[path = "admission/turn_state.rs"]
mod turn_state;
