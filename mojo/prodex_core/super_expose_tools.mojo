# MCP tool catalog, annotations, mode selection and optional-alias descriptions.
from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_STRING,
    pj_valid, pj_kind, pj_child, pj_next, pj_text, pj_literal, pj_equal,
)
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr
from json_sink import JsonSink, js_byte, js_literal, js_string, js_view, js_escaped, js_u64
from super_expose import (
    super_expose_tool, super_expose_tool_is_allowed, SUPER_EXPOSE_TOOL_EXEC,
    SUPER_EXPOSE_MODE_FULL, SUPER_EXPOSE_MODE_EXEC,
)

comptime TOOL_READ_ONLY: Int64 = 1
comptime TOOL_DESTRUCTIVE: Int64 = 2
comptime TOOL_OPEN_WORLD: Int64 = 4


@fieldwise_init
struct ToolDefinition(Copyable):
    var name: ProdexRichStringView
    var description: ProdexRichStringView
    var input_schema: ProdexRichStringView
    var output_schema: ProdexRichStringView
    var annotations: Int64


def run_id_schema() -> ProdexRichStringView:
    return pj_literal(StringSlice('{"type":"object","properties":{"run_id":{"type":"string","minLength":1,"maxLength":128}},"required":["run_id"],"additionalProperties":false}'))


def status_schema() -> ProdexRichStringView:
    return pj_literal(StringSlice('{"type":"object","properties":{"instance_id":{"type":"string"},"run_id":{"type":"string"},"state":{"type":"string"},"created_at":{"type":"integer"},"started_at":{"type":["integer","null"]},"finished_at":{"type":["integer","null"]},"exit_status":{"type":["integer","null"]},"provider":{"type":["string","null"]},"model":{"type":["string","null"]},"reasoning_effort":{"type":["string","null"]},"cancellation_requested":{"type":"boolean"}},"required":["instance_id","run_id","state"]}'))


def tool_definition(index: Int64) -> ToolDefinition:
    if index == 0:
        return ToolDefinition(
            pj_literal(StringSlice("prodex_super_start")),
            pj_literal(StringSlice("Start one full-access Prodex Super task in the captured initial working directory. The task retains normal OS-user filesystem, process, network, Git, and local-tool authority; the initial directory is not a jail. Use only for explicit user-requested development work; poll its run_id instead of starting duplicates.")),
            pj_literal(StringSlice('{"type":"object","properties":{"task":{"type":"string","minLength":1,"maxLength":65536},"model":{"type":["string","null"],"maxLength":256},"reasoning_effort":{"type":["string","null"],"maxLength":256},"provider":{"type":["string","null"],"maxLength":256},"profile":{"type":["string","null"],"maxLength":128},"sub_agents":{"type":["boolean","null"]}},"required":["task"],"additionalProperties":false}')),
            pj_literal(StringSlice('{"type":"object","properties":{"run_id":{"type":"string"},"state":{"type":"string"}},"required":["run_id","state"]}')),
            TOOL_DESTRUCTIVE | TOOL_OPEN_WORLD,
        )
    if index == 1:
        return ToolDefinition(
            pj_literal(StringSlice("prodex_super_status")),
            pj_literal(StringSlice("Read the current bounded state of one Prodex Super run; always provide its explicit run_id and poll this instead of starting a duplicate.")),
            run_id_schema(), status_schema(), TOOL_READ_ONLY,
        )
    if index == 2:
        return ToolDefinition(
            pj_literal(StringSlice("prodex_super_events")),
            pj_literal(StringSlice("Read a bounded monotonic page of redacted stdout/stderr lifecycle events for one explicit run_id.")),
            pj_literal(StringSlice('{"type":"object","properties":{"run_id":{"type":"string"},"after_seq":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":')),
            pj_literal(StringSlice('{"type":"object","properties":{"instance_id":{"type":"string"},"run_id":{"type":"string"},"events":{"type":"array"},"next_seq":{"type":"integer"},"truncated":{"type":"boolean"}},"required":["instance_id","run_id","events","next_seq","truncated"]}')),
            TOOL_READ_ONLY,
        )
    if index == 3:
        return ToolDefinition(
            pj_literal(StringSlice("prodex_super_result")),
            pj_literal(StringSlice("Read a bounded final result for one explicit Prodex Super run, or its current nonterminal state.")),
            run_id_schema(),
            pj_literal(StringSlice('{"type":"object","properties":{"instance_id":{"type":"string"},"run_id":{"type":"string"},"state":{"type":"string"},"output":{"type":"string"},"output_truncated":{"type":"boolean"}},"required":["instance_id","run_id","state"]}')),
            TOOL_READ_ONLY,
        )
    if index == 4:
        return ToolDefinition(
            pj_literal(StringSlice("prodex_super_cancel")),
            pj_literal(StringSlice("Cancel one Prodex Super run and terminate only its child process tree.")),
            run_id_schema(), status_schema(), TOOL_DESTRUCTIVE,
        )
    if index == 5:
        return ToolDefinition(
            pj_literal(StringSlice("prodex_super_list")),
            pj_literal(StringSlice("List bounded runs owned by this expose instance; it never lists runs from another instance.")),
            pj_literal(StringSlice('{"type":"object","properties":{},"additionalProperties":false}')),
            pj_literal(StringSlice('{"type":"object","properties":{"instance_id":{"type":"string"},"runs":{"type":"array"}},"required":["instance_id","runs"]}')),
            TOOL_READ_ONLY,
        )
    if index == 6:
        return ToolDefinition(
            pj_literal(StringSlice("prodex_super_exec")),
            pj_literal(StringSlice("Execute one direct OS command under the expose process's current local OS-user authority. It is synchronous and standalone: it does not require, create, attach to, or depend on a Prodex Super run or plain prodex s session. Use an explicit shell executable such as sh or cmd.exe when shell syntax is needed.")),
            pj_literal(StringSlice('{"type":"object","properties":{"program":{"type":"string","minLength":1,"maxLength":4096},"args":{"type":["array","null"],"maxItems":256,"items":{"type":"string","maxLength":16384}},"cwd":{"type":["string","null"],"maxLength":4096},"env":{"type":["object","null"],"maxProperties":128,"additionalProperties":{"type":"string","maxLength":16384},"propertyNames":{"maxLength":256}},"stdin":{"type":["string","null"],"maxLength":262144},"timeout_ms":{"type":["integer","null"],"minimum":1,"maximum":120000,"default":30000}},"required":["program"],"additionalProperties":false,"x-maxTotalArgumentBytes":262144}')),
            pj_literal(StringSlice('{"type":"object","properties":{"status":{"type":"string","enum":["completed","timed_out","cancelled"]},"program":{"type":"string"},"optional_tool":{"type":["string","null"]},"available_optional_tools":{"type":"array","items":{"type":"string"}},"arg_count":{"type":"integer"},"cwd":{"type":"string"},"pid":{"type":["integer","null"]},"exit_code":{"type":["integer","null"]},"exit_status":{"type":"integer"},"signal":{"type":["integer","null"]},"termination":{"type":["string","null"]},"success":{"type":"boolean"},"duration_ms":{"type":"integer"},"stdout":{"type":"string","maxLength":131072},"stdout_truncated":{"type":"boolean"},"stderr":{"type":"string","maxLength":131072},"stderr_truncated":{"type":"boolean"}},"required":["status","program","optional_tool","available_optional_tools","arg_count","cwd","pid","exit_code","exit_status","signal","termination","success","duration_ms","stdout","stdout_truncated","stderr","stderr_truncated"]}')),
            TOOL_DESTRUCTIVE | TOOL_OPEN_WORLD,
        )
    if index == 7:
        return ToolDefinition(
            pj_literal(StringSlice("prodex_session_prompt_write")),
            pj_literal(StringSlice("Prompt Write: deliver one session input to an already-running plain prodex s through the supported Codex control plane. It uses the same fail-closed identity checks as output reads and never starts another solver. A returned output_cursor is an optional pre-write rollout anchor; write_ambiguous means delivery may have happened and must not be replayed.")),
            pj_literal(StringSlice('{"type":"object","properties":{"message":{"type":"string","minLength":1,"maxLength":65536},"cwd":{"type":["string","null"],"maxLength":4096},"prodex_pid":{"type":["integer","null"],"minimum":1},"thread_id":{"type":["string","null"],"maxLength":128}},"required":["message"],"additionalProperties":false}')),
            pj_literal(StringSlice('{"type":"object","properties":{"status":{"type":"string"},"prodex_pid":{"type":"integer"},"codex_pid":{"type":"integer"},"thread_id":{"type":"string"},"message_id":{"type":["string","null"]},"submission_id":{"type":["string","null"]},"output_cursor":{"type":["string","null"]},"queue_exit":{"type":"integer"},"verification":{"type":"string"},"recovery_generation":{"type":"integer"},"last_prompt_requeued":{"type":"boolean"},"requeue_reason":{"type":["string","null"]}},"required":["status","prodex_pid","codex_pid","thread_id","message_id","submission_id","output_cursor","queue_exit","verification","recovery_generation","last_prompt_requeued","requeue_reason"]}')),
            0,
        )
    if index == 8:
        return ToolDefinition(
            pj_literal(StringSlice("prodex_session_preempt")),
            pj_literal(StringSlice("Preempt the current turn for one exact existing plain prodex s session, then remove every still-pending queued prompt through Codex. It never kills a process or reports an already-started prompt as cancelled; ambiguous queue or interrupt state fails closed.")),
            pj_literal(StringSlice('{"type":"object","properties":{"cwd":{"type":["string","null"],"maxLength":4096},"prodex_pid":{"type":["integer","null"],"minimum":1},"thread_id":{"type":["string","null"],"maxLength":128}},"additionalProperties":false}')),
            pj_literal(StringSlice('{"type":"object","properties":{"status":{"type":"string"},"preempted":{"type":"boolean"},"prodex_pid":{"type":"integer"},"codex_pid":{"type":"integer"},"thread_id":{"type":"string"},"current_turn_id":{"type":["string","null"]},"current_turn_found":{"type":"boolean"},"current_turn_interrupted":{"type":"boolean"},"cancelled_submission_ids":{"type":"array","items":{"type":"string"}},"cancelled_count":{"type":"integer"},"remaining_submission_ids":{"type":"array","items":{"type":"string"}},"remaining_count":{"type":"integer"},"queue_empty_at_boundary":{"type":"boolean"},"session_ready":{"type":"boolean"},"generation":{"type":"integer"},"generation_boundary":{"type":"integer"}},"required":["status","preempted","prodex_pid","codex_pid","thread_id","current_turn_id","current_turn_found","current_turn_interrupted","cancelled_submission_ids","cancelled_count","remaining_submission_ids","remaining_count","queue_empty_at_boundary","session_ready","generation","generation_boundary"]}')),
            TOOL_DESTRUCTIVE,
        )
    return ToolDefinition(
        pj_literal(StringSlice("prodex_session_output_read")),
        pj_literal(StringSlice("Read bounded user-visible output from the same already-running plain prodex s interactive session; it never starts another solver or reads its PTY. Pages may contain generic gap markers for safely skipped malformed records and text_truncated markers for bounded text.")),
        pj_literal(StringSlice('{"type":"object","properties":{"cursor":{"type":["string","null"],"maxLength":16384},"limit":{"type":"integer","minimum":1,"maximum":200},"wait_ms":{"type":"integer","minimum":0,"maximum":10000},"prodex_pid":{"type":["integer","null"],"minimum":1},"thread_id":{"type":["string","null"],"maxLength":128}},"additionalProperties":false}')),
        pj_literal(StringSlice('{"type":"object","properties":{"status":{"type":"string"},"prodex_pid":{"type":"integer"},"codex_pid":{"type":"integer"},"thread_id":{"type":"string"},"source":{"type":"string"},"events":{"type":"array"},"next_cursor":{"type":"string"},"has_more":{"type":"boolean"}},"required":["status","prodex_pid","codex_pid","thread_id","source","events","next_cursor","has_more"]}')),
        TOOL_READ_ONLY,
    )


def write_alias_description(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson):
    var item = pj_child(tree, 0)
    if item < 0:
        js_literal(sink, StringSlice(" No validated Prodex optional-tool aliases are active for this endpoint."))
        return
    js_literal(sink, StringSlice(" Validated optional-tool aliases available at endpoint start: "))
    var emitted = False
    while item >= 0:
        if emitted:
            js_literal(sink, StringSlice(", "))
        js_literal(sink, StringSlice("optional:"))
        var name = pj_text(tree, item)
        js_escaped(sink, name)
        if pj_equal(name, pj_literal(StringSlice("playwright-mcp"))):
            js_literal(sink, StringSlice(", optional:playwright"))
        emitted = True
        item = pj_next(tree, item)
    js_literal(sink, StringSlice(". Prefer these aliases over guessing PATH locations."))


def write_tool(
    sink: Pointer[mut=True, JsonSink, _], definition: ToolDefinition,
    tree: ParsedJson, index: Int64, max_event_page: Int64,
):
    js_literal(sink, StringSlice('{"name":'))
    js_string(sink, definition.name)
    js_literal(sink, StringSlice(',"title":"'))
    var name = rich_view_ptr(definition.name)
    for position in range(Int64(definition.name.len)):
        var byte = name[unsafe_offset=position]
        js_byte(sink, UInt8(32) if byte == 95 else byte)
    js_literal(sink, StringSlice('","description":"'))
    js_escaped(sink, definition.description)
    if super_expose_tool(definition.name) == SUPER_EXPOSE_TOOL_EXEC:
        write_alias_description(sink, tree)
    js_literal(sink, StringSlice('","inputSchema":'))
    js_view(sink, definition.input_schema)
    if index == 2:
        # This schema's maximum follows the host's existing event-page bound.
        js_u64(sink, UInt64(max_event_page))
        js_literal(sink, StringSlice('}},"required":["run_id"],"additionalProperties":false}'))
    js_literal(sink, StringSlice(',"outputSchema":'))
    js_view(sink, definition.output_schema)
    js_literal(sink, StringSlice(',"annotations":{"readOnlyHint":'))
    js_literal(sink, StringSlice("true") if definition.annotations & TOOL_READ_ONLY != 0 else StringSlice("false"))
    js_literal(sink, StringSlice(',"destructiveHint":'))
    js_literal(sink, StringSlice("true") if definition.annotations & TOOL_DESTRUCTIVE != 0 else StringSlice("false"))
    js_literal(sink, StringSlice(',"openWorldHint":'))
    js_literal(sink, StringSlice("true") if definition.annotations & TOOL_OPEN_WORLD != 0 else StringSlice("false"))
    js_literal(sink, StringSlice("}}"))


@export("prodex_mojo_super_expose_tool_catalog_v1")
def prodex_mojo_super_expose_tool_catalog_v1(
    abi: Int64, max_event_page: Int64, exec_only: Int64,
    nodes_address: UInt64, count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64,
    measuring: Int64, output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if max_event_page <= 0 or exec_only < 0 or exec_only > 1 or count <= 0 or nodes_address == 0 or raw_length < 0 or raw_length > 65_536 or scratch_count < count or scratch_address == 0 or metadata_address == 0 or capacity < 0 or measuring < 0 or measuring > 1:
        return 1
    if measuring == 1 and (output_address != 0 or capacity != 0) or measuring == 0 and output_address == 0:
        return 1
    var tree = ParsedJson(Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)), count, ProdexRichStringView(UInt(raw_address), UInt(raw_length)))
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_ARRAY:
        return 1
    var alias = pj_child(tree, 0)
    while alias >= 0:
        if pj_kind(tree, alias) != JSON_STRING:
            return 1
        alias = pj_next(tree, alias)
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, measuring == 1, False)
    var mode = SUPER_EXPOSE_MODE_EXEC if exec_only == 1 else SUPER_EXPOSE_MODE_FULL
    js_literal(Pointer(to=sink), StringSlice('{"tools":['))
    var emitted: Int64 = 0
    var exec_index: Int64 = -1
    for index in range(10):
        var definition = tool_definition(Int64(index))
        var kind = super_expose_tool(definition.name)
        if not super_expose_tool_is_allowed(mode, kind):
            continue
        if emitted > 0:
            js_byte(Pointer(to=sink), 44)
        if kind == SUPER_EXPOSE_TOOL_EXEC:
            exec_index = emitted
        write_tool(Pointer(to=sink), definition, tree, Int64(index), max_event_page)
        emitted += 1
    if exec_index < 0:
        return 1
    js_literal(Pointer(to=sink), StringSlice('],"exec_index":'))
    js_u64(Pointer(to=sink), UInt64(exec_index))
    js_byte(Pointer(to=sink), 125)
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = sink.written
    return 0
