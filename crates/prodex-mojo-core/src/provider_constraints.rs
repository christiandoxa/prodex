mod gemini_bridge_request;
mod gemini_request;
mod gemini_request_content;
mod gemini_sse_tool_call_index;

pub use gemini_bridge_request::{
    GeminiBridgeRequestKernelInput, GeminiBridgeRequestOperation, gemini_bridge_request_kernel,
};
pub use gemini_request::{
    GEMINI_REQUEST_FIELD_PLAN_MAX_FIELDS, GeminiRequestField, GeminiRequestFieldTarget,
    gemini_request_field_plan,
};
pub use gemini_request_content::{
    GeminiRequestContentKernelInput, GeminiRequestContentOperation, gemini_request_content_kernel,
};
pub use gemini_sse_tool_call_index::{
    GeminiToolCallIndexBinding, GeminiToolCallIndexRecord, gemini_tool_call_index,
};

pub fn self_test() -> bool {
    gemini_request_field_plan(0, 0, 0).is_ok_and(|fields| fields.is_empty())
}
