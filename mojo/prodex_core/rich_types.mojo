from std.memory import Pointer


@fieldwise_init
struct ProdexRichStringView(Copyable):
    # Raw address is deliberate: this record is passed by value through C.
    var ptr: UInt
    var len: UInt


def rich_view_ptr(
    view: ProdexRichStringView,
) -> Pointer[mut=False, UInt8, ImmUntrackedOrigin]:
    return Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(view.ptr)
    )


@fieldwise_init
struct ProdexRichSlice(Copyable):
    var offset: Int64
    var len: Int64


@fieldwise_init
struct ProdexRichIssue(Copyable):
    var domain: Int64
    var kind: Int64
    var field: Int64
    var object_index: Int64
    var byte_offset: Int64
    var byte_length: Int64
    var expected: Int64


@fieldwise_init
struct ProdexRichContextRecord(Copyable):
    var key: ProdexRichSlice
    var kind: Int64
    var severity: Int64
    var first_line: Int64
    var occurrences: Int64
    var token_count: Int64
    var duplicate_count: Int64


@fieldwise_init
struct ProdexRichContextResult(Copyable):
    var abi_version: Int64
    var line_count: Int64
    var records_written: Int64
    var required_records: Int64
    var output_written: Int64
    var required_output: Int64
    var required_scratch: Int64
    var issue_kind: Int64
    var issue_offset: Int64
    var issue_length: Int64
    var counts: InlineArray[Int64, 7]
    var noise_lines: Int64
    var signal_lines: Int64
    var token_count: Int64


@fieldwise_init
struct ProdexRichRouteInput(Copyable):
    var provider: ProdexRichStringView
    var model: ProdexRichStringView
    var capabilities: ProdexRichStringView
    var hard_eligible: Int64
    var health: Int64
    var load: Int64
    var quota_headroom: Int64
    var quota_present: Int64
    var cost: Int64
    var latency: Int64
    var risk: Int64
    var priority: Int64
    var affinity: Int64


@fieldwise_init
struct ProdexRichRouteRecord(Copyable):
    var provider: ProdexRichSlice
    var model: ProdexRichSlice
    var capability_mask: Int64
    var eligible: Int64
    var reason: Int64
    var score: Int64
    var components: InlineArray[Int64, 7]
    var weighted_total: Int64
    var input_index: Int64
    var duplicate_of: Int64
    var provider_order: Int64
    var affinity: Int64


@fieldwise_init
struct ProdexRichRouteResult(Copyable):
    var abi_version: Int64
    var candidates_written: Int64
    var required_candidates: Int64
    var ordered_written: Int64
    var selected_index: Int64
    var output_written: Int64
    var required_output: Int64
    var issue_kind: Int64
    var issue_offset: Int64
    var issue_length: Int64


@fieldwise_init
struct ProdexRichPolicyInput(Copyable):
    var alias_view: ProdexRichStringView
    var models: UInt
    var model_count: Int64
    var strategy: ProdexRichStringView
    var metrics: UInt
    var metric_count: Int64


@fieldwise_init
struct ProdexRichPolicyModel(Copyable):
    var model: ProdexRichSlice
    var model_index: Int64
    var metric_match: Int64


@fieldwise_init
struct ProdexRichPolicyResult(Copyable):
    var abi_version: Int64
    var models_written: Int64
    var required_models: Int64
    var output_written: Int64
    var required_output: Int64
    var issue_kind: Int64
    var issue_field: Int64
    var issue_index: Int64
    var issue_offset: Int64
    var issue_length: Int64


@fieldwise_init
struct ProdexRichCatalogReasoningResult(Copyable):
    var abi_version: Int64
    var model_index: Int64
    var efforts_written: Int64
    var selected_effort: ProdexRichSlice
    var default_effort: ProdexRichSlice
    var output_written: Int64
    var issue_kind: Int64
    var issue_index: Int64
    var issue_offset: Int64
    var issue_length: Int64


@fieldwise_init
struct ProdexRichPolicyRouteInput(Copyable):
    var model: ProdexRichStringView
    var input_cost: UInt64
    var input_cost_present: Int64
    var output_cost: UInt64
    var output_cost_present: Int64
    var policy_latency: UInt64
    var policy_latency_present: Int64
    var state_latency: UInt64
    var state_latency_present: Int64
    var in_flight: UInt64
    var rpm_limit: UInt64
    var rpm_limit_present: Int64
    var rpm_used: UInt64
    var tpm_limit: UInt64
    var tpm_limit_present: Int64
    var tpm_used: UInt64


@fieldwise_init
struct ProdexRichPolicyRouteResult(Copyable):
    var abi_version: Int64
    var selected_index: Int64
    var ordered_written: Int64
    var required_ordered: Int64
    var issue_kind: Int64
    var issue_index: Int64
    var issue_offset: Int64
    var issue_length: Int64


@fieldwise_init
struct ProdexRichFallbackRecord(Copyable):
    var model: ProdexRichSlice
    var source_kind: Int64
    var input_index: Int64


@fieldwise_init
struct ProdexRichFallbackResult(Copyable):
    var abi_version: Int64
    var records_written: Int64
    var required_records: Int64
    var output_written: Int64
    var required_output: Int64
    var issue_kind: Int64
    var issue_offset: Int64
    var issue_length: Int64


@fieldwise_init
struct ProdexRichPlanItem(Copyable):
    var id: ProdexRichStringView
    var token_cost: Int64
    var required: Int64


@fieldwise_init
struct ProdexRichPlanAction(Copyable):
    var id: ProdexRichSlice
    var action: Int64
    var reason: Int64
    var token_cost: Int64
    var input_index: Int64


@fieldwise_init
struct ProdexRichPlanResult(Copyable):
    var abi_version: Int64
    var actions_written: Int64
    var required_actions: Int64
    var output_written: Int64
    var required_output: Int64
    var used_tokens: Int64
    var issue_kind: Int64
    var issue_offset: Int64
    var issue_length: Int64


@fieldwise_init
struct NormalizedIdentifier(Copyable):
    var raw: ProdexRichStringView
    var normalized: ProdexRichSlice


@fieldwise_init
struct RouteCandidate(Copyable):
    var provider: NormalizedIdentifier
    var model: NormalizedIdentifier
    var capability_mask: Int64
    var eligible: Int64
    var score: Int64
    var components: InlineArray[Int64, 7]
    var weighted_total: Int64
    var input_index: Int64


@fieldwise_init
struct RouteScore(Copyable):
    var components: InlineArray[Int64, 7]
    var weighted_total: Int64
    var score: Int64


@fieldwise_init
struct PolicyRule(Copyable):
    var alias_slice: ProdexRichSlice
    var model: ProdexRichSlice
    var metric_match: Int64


@fieldwise_init
struct ContextItem(Copyable):
    var id: ProdexRichStringView
    var token_cost: Int64
    var required: Int64
    var available: Int64


@fieldwise_init
struct ContextPlan(Copyable):
    var action_count: Int64
    var used_tokens: Int64
