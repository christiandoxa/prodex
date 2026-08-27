//! Rich, borrowed-record Mojo operations.
//!
//! The ABI is deliberately boring: Rust owns the input and output storage,
//! Mojo owns parsing and deterministic decisions for the duration of one call,
//! and only validated offset-based records cross back. No Mojo or Rust heap
//! object is shared across this boundary.

use crate::{MojoError, MojoIssue};

// v3 uses fixed-width UInt64 address arguments for nullable C ABI pointers.
// Mojo Optional[Pointer] is not a portable C ABI representation on Windows.
pub const RICH_ABI_VERSION: i64 = 3;

const _: () = assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u64>());

mod routing;
pub use routing::{RouteCandidate, RouteInput, RoutePlan, plan_routes};
mod context_plan;
pub use context_plan::{ContextPlan, ContextPlanAction, ContextPlanItem, plan_context_items};
mod context;
pub use context::{ContextAnalysis, ContextGroup, analyze_context};
mod policy;
pub use policy::{PolicyAliasInput, PolicyAliasPlan, PolicyModel, validate_policy_alias};
mod fallback;
pub use fallback::model_fallback_chain;

const RICH_STATUS_INVALID: i64 = 1;
const RICH_STATUS_UTF8: i64 = 2;
const RICH_STATUS_CAPACITY: i64 = 3;
const RICH_STATUS_ABI: i64 = 4;
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichStringView {
    ptr: *const u8,
    len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichSlice {
    offset: i64,
    len: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichContextRecord {
    key: RichSlice,
    kind: i64,
    severity: i64,
    first_line: i64,
    occurrences: i64,
    token_count: i64,
    duplicate_count: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichContextResult {
    abi_version: i64,
    line_count: i64,
    records_written: i64,
    required_records: i64,
    output_written: i64,
    required_output: i64,
    required_scratch: i64,
    issue_kind: i64,
    issue_offset: i64,
    issue_length: i64,
    counts: [i64; 7],
    noise_lines: i64,
    signal_lines: i64,
    token_count: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichRouteInput {
    provider: RichStringView,
    model: RichStringView,
    capabilities: RichStringView,
    hard_eligible: i64,
    health: i64,
    load: i64,
    quota_headroom: i64,
    quota_present: i64,
    cost: i64,
    latency: i64,
    risk: i64,
    priority: i64,
    affinity: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichRouteRecord {
    provider: RichSlice,
    model: RichSlice,
    capability_mask: i64,
    eligible: i64,
    reason: i64,
    score: i64,
    components: [i64; 7],
    weighted_total: i64,
    input_index: i64,
    duplicate_of: i64,
    provider_order: i64,
    affinity: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichRouteResult {
    abi_version: i64,
    candidates_written: i64,
    required_candidates: i64,
    ordered_written: i64,
    selected_index: i64,
    output_written: i64,
    required_output: i64,
    issue_kind: i64,
    issue_offset: i64,
    issue_length: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichPolicyInput {
    alias_view: RichStringView,
    models: *const RichStringView,
    model_count: i64,
    strategy: RichStringView,
    metrics: *const RichStringView,
    metric_count: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichPolicyModel {
    model: RichSlice,
    model_index: i64,
    metric_match: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichPolicyResult {
    abi_version: i64,
    models_written: i64,
    required_models: i64,
    output_written: i64,
    required_output: i64,
    issue_kind: i64,
    issue_field: i64,
    issue_index: i64,
    issue_offset: i64,
    issue_length: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichFallbackRecord {
    model: RichSlice,
    source_kind: i64,
    input_index: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichFallbackResult {
    abi_version: i64,
    records_written: i64,
    required_records: i64,
    output_written: i64,
    required_output: i64,
    issue_kind: i64,
    issue_offset: i64,
    issue_length: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichPlanItem {
    id: RichStringView,
    token_cost: i64,
    required: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichPlanAction {
    id: RichSlice,
    action: i64,
    reason: i64,
    token_cost: i64,
    input_index: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct RichPlanResult {
    abi_version: i64,
    actions_written: i64,
    required_actions: i64,
    output_written: i64,
    required_output: i64,
    used_tokens: i64,
    issue_kind: i64,
    issue_offset: i64,
    issue_length: i64,
}

const _: () = {
    assert!(std::mem::size_of::<RichStringView>() == 16);
    assert!(std::mem::size_of::<RichSlice>() == 16);
    assert!(std::mem::size_of::<RichContextRecord>() == 64);
    assert!(std::mem::size_of::<RichContextResult>() == 160);
    assert!(std::mem::size_of::<RichRouteInput>() == 128);
    assert!(std::mem::size_of::<RichRouteRecord>() == 160);
    assert!(std::mem::size_of::<RichRouteResult>() == 80);
    assert!(std::mem::size_of::<RichPolicyInput>() == 64);
    assert!(std::mem::size_of::<RichPolicyModel>() == 32);
    assert!(std::mem::size_of::<RichPolicyResult>() == 80);
    assert!(std::mem::size_of::<RichPlanItem>() == 32);
    assert!(std::mem::size_of::<RichPlanAction>() == 48);
    assert!(std::mem::size_of::<RichPlanResult>() == 72);
};

// Exported Mojo functions receive caller-owned pointer addresses as u64. The
// Mojo side validates zero before reconstructing a typed pointer.
unsafe extern "C" {
    fn prodex_mojo_rich_abi_version() -> i64;
    fn prodex_mojo_rich_abi_layout(output: *mut u64, output_count: i64) -> i64;
    fn prodex_mojo_rich_context_analyze_v2(
        abi_version: i64,
        input: RichStringView,
        output_records: u64,
        record_capacity: i64,
        output: u64,
        output_capacity: i64,
        hash_slots: u64,
        hash_capacity: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_rich_route_plan_v2(
        abi_version: i64,
        inputs: u64,
        input_count: i64,
        required_capabilities: RichStringView,
        output_records: u64,
        record_capacity: i64,
        ordered_indices: u64,
        ordered_capacity: i64,
        output: u64,
        output_capacity: i64,
        hash_slots: u64,
        hash_capacity: i64,
        health_weight: i64,
        load_weight: i64,
        cost_weight: i64,
        latency_weight: i64,
        risk_weight: i64,
        priority_weight: i64,
        affinity_weight: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_rich_policy_alias_v2(
        abi_version: i64,
        input: RichPolicyInput,
        output_models: u64,
        model_capacity: i64,
        output: u64,
        output_capacity: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_rich_model_fallback_v2(
        abi_version: i64,
        provider: RichStringView,
        model: RichStringView,
        output_records: u64,
        record_capacity: i64,
        output: u64,
        output_capacity: i64,
        hash_slots: u64,
        hash_capacity: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_rich_context_plan_v2(
        abi_version: i64,
        items: u64,
        item_count: i64,
        available: u64,
        available_count: i64,
        token_budget: i64,
        tier: i64,
        output_actions: u64,
        action_capacity: i64,
        output: u64,
        output_capacity: i64,
        hash_slots: u64,
        hash_capacity: i64,
        result: u64,
    ) -> i64;
}

#[inline]
fn mojo_pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

#[inline]
fn mojo_mut_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}

static RICH_ABI_READY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

fn view(value: &str) -> RichStringView {
    RichStringView {
        ptr: value.as_ptr(),
        len: value.len(),
    }
}

fn hash_capacity(count: usize) -> Result<usize, MojoError> {
    count
        .checked_mul(2)
        .and_then(|value| value.max(1).checked_next_power_of_two())
        .ok_or(MojoError::InvalidInput)
}

fn rich_abi_ready() -> bool {
    *RICH_ABI_READY.get_or_init(|| {
        let mut layout = [0_u64; 28];
        let status =
            unsafe { prodex_mojo_rich_abi_layout(layout.as_mut_ptr(), layout.len() as i64) };
        let rust = [
            std::mem::size_of::<RichStringView>() as u64,
            std::mem::align_of::<RichStringView>() as u64,
            std::mem::size_of::<RichSlice>() as u64,
            std::mem::align_of::<RichSlice>() as u64,
            std::mem::size_of::<MojoIssue>() as u64,
            std::mem::align_of::<MojoIssue>() as u64,
            std::mem::size_of::<RichContextRecord>() as u64,
            std::mem::align_of::<RichContextRecord>() as u64,
            std::mem::size_of::<RichContextResult>() as u64,
            std::mem::align_of::<RichContextResult>() as u64,
            std::mem::size_of::<RichRouteInput>() as u64,
            std::mem::align_of::<RichRouteInput>() as u64,
            std::mem::size_of::<RichRouteRecord>() as u64,
            std::mem::align_of::<RichRouteRecord>() as u64,
            std::mem::size_of::<RichRouteResult>() as u64,
            std::mem::align_of::<RichRouteResult>() as u64,
            std::mem::size_of::<RichPolicyInput>() as u64,
            std::mem::align_of::<RichPolicyInput>() as u64,
            std::mem::size_of::<RichPolicyModel>() as u64,
            std::mem::align_of::<RichPolicyModel>() as u64,
            std::mem::size_of::<RichPolicyResult>() as u64,
            std::mem::align_of::<RichPolicyResult>() as u64,
            std::mem::size_of::<RichPlanItem>() as u64,
            std::mem::align_of::<RichPlanItem>() as u64,
            std::mem::size_of::<RichPlanAction>() as u64,
            std::mem::align_of::<RichPlanAction>() as u64,
            std::mem::size_of::<RichPlanResult>() as u64,
            std::mem::align_of::<RichPlanResult>() as u64,
        ];
        status == 0
            && unsafe { prodex_mojo_rich_abi_version() } == RICH_ABI_VERSION
            && layout == rust
    })
}

fn issue(domain: i64, kind: i64, field: i64, index: i64, offset: i64, length: i64) -> MojoError {
    MojoError::Structured(MojoIssue {
        domain,
        kind,
        field,
        object_index: index,
        byte_offset: offset,
        byte_length: length,
        expected: 0,
    })
}

fn status_error(status: i64, domain: i64, kind: i64, offset: i64, length: i64) -> MojoError {
    match status {
        RICH_STATUS_ABI => MojoError::AbiMismatch,
        RICH_STATUS_CAPACITY => MojoError::Capacity,
        RICH_STATUS_UTF8 => issue(domain, kind, 0, -1, offset, length),
        RICH_STATUS_INVALID => MojoError::InvalidInput,
        _ => MojoError::InvalidOutput,
    }
}

fn slice(output: &[u8], value: RichSlice) -> Result<&[u8], MojoError> {
    let offset = usize::try_from(value.offset).map_err(|_| MojoError::InvalidOutput)?;
    let length = usize::try_from(value.len).map_err(|_| MojoError::InvalidOutput)?;
    let end = offset.checked_add(length).ok_or(MojoError::InvalidOutput)?;
    output.get(offset..end).ok_or(MojoError::InvalidOutput)
}

fn ensure_rich_abi() -> Result<(), MojoError> {
    rich_abi_ready().then_some(()).ok_or(MojoError::AbiMismatch)
}

pub fn rich_self_test() -> bool {
    let context = analyze_context(" error: 火\r\nnoise\nerror: 火\n").is_ok_and(|value| {
        value.counts[0] == 2 && value.groups.len() == 1 && value.groups[0].duplicate_count == 1
    });
    let routes = plan_routes(
        &[RouteInput {
            provider: " OpenAI ",
            model: "gpt-5",
            capabilities: "responses_api,tools",
            hard_eligible: true,
            health: 10_000,
            load: 0,
            quota_headroom: Some(10_000),
            cost: 0,
            latency: 0,
            risk: 0,
            priority: 10_000,
            affinity: false,
        }],
        "responses_api",
        [10_000, 0, 0, 0, 0, 0, 0],
    )
    .is_ok_and(|value| value.selected_index == Some(0) && value.candidates[0].provider == "openai");
    let policy = validate_policy_alias(PolicyAliasInput {
        alias: "prodex",
        models: &["gpt-5"],
        strategy: Some("ordered-fallback"),
        metrics: &["gpt-5"],
    })
    .is_ok_and(|value| value.models[0].metric_match == Some(0));
    let fallback = model_fallback_chain("copilot", " codex ")
        .is_ok_and(|value| value == ["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"]);
    let context_plan = plan_context_items(
        &[ContextPlanItem {
            id: "psc:example#L1-L2",
            token_cost: 3,
            required: true,
        }],
        &["psc:example#L1-L2"],
        4,
        3,
    )
    .is_ok_and(|value| value.actions[0].action == 1 && value.used_tokens == 3);
    let result = context && routes && policy && fallback && context_plan;
    if !result && std::env::var_os("PRODEX_MOJO_SELF_TEST_TRACE").is_some() {
        eprintln!(
            "Mojo rich self-test: context={context} routes={routes} policy={policy} fallback={fallback} context_plan={context_plan}"
        );
    }
    result
}

#[cfg(test)]
#[path = "rich_tests.rs"]
mod tests;
