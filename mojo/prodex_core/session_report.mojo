from std.memory import Pointer

from parsed_json import (
    JSON_OBJECT,
    ParsedJson,
    ParsedJsonNode,
    pj_field,
    pj_is,
    pj_kind,
    pj_nonblank,
    pj_string_field,
    pj_valid,
)
from rich_types import ProdexRichStringView

comptime SESSION_REPORT_ABI_VERSION: Int64 = 1
comptime SESSION_REPORT_OK: Int64 = 0
comptime SESSION_REPORT_INVALID: Int64 = 1

def session_string_field(tree: ParsedJson, object: Int64, name: StringSlice) -> Int64:
    var field = pj_string_field(tree, object, name)
    return field if field >= 0 and pj_nonblank(tree, field) else -1

def session_object_field(tree: ParsedJson, object: Int64, name: StringSlice) -> Int64:
    var field = pj_field(tree, object, name)
    return field if pj_kind(tree, field) == JSON_OBJECT else -1

def session_nested_string2(
    tree: ParsedJson,
    object: Int64,
    parent: StringSlice,
    name: StringSlice,
) -> Int64:
    var nested = session_object_field(tree, object, parent)
    return session_string_field(tree, nested, name) if nested >= 0 else -1

def session_nested_string5(
    tree: ParsedJson,
    object: Int64,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
    fourth: StringSlice,
    name: StringSlice,
) -> Int64:
    var one = session_object_field(tree, object, first)
    if one < 0:
        return -1
    var two = session_object_field(tree, one, second)
    if two < 0:
        return -1
    var three = session_object_field(tree, two, third)
    if three < 0:
        return -1
    var four = session_object_field(tree, three, fourth)
    return session_string_field(tree, four, name) if four >= 0 else -1

@export("prodex_session_report_metadata_v1")
def prodex_session_report_metadata_v1(
    abi_version: Int64,
    nodes_address: UInt,
    nodes_count: Int64,
    raw_address: UInt,
    raw_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_REPORT_ABI_VERSION
        or nodes_address == 0
        or nodes_count <= 0
        or raw_length < 0
        or output_address == 0
        or (raw_length > 0 and raw_address == 0)
    ):
        return SESSION_REPORT_INVALID

    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](
            unsafe_from_address=Int(nodes_address)
        ),
        nodes_count,
        ProdexRichStringView(UInt(raw_address), UInt(raw_length)),
    )
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_OBJECT:
        return SESSION_REPORT_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(9):
        output[unsafe_offset=index] = -1

    var payload = session_object_field(tree, 0, StringSlice("payload"))
    var metadata = session_object_field(tree, 0, StringSlice("metadata"))
    var payload_metadata = (
        session_object_field(tree, payload, StringSlice("metadata"))
        if payload >= 0
        else -1
    )

    var type_node = pj_string_field(tree, 0, StringSlice("type"))
    var type_class: Int64 = 0
    if type_node >= 0:
        if pj_is["session_meta"](tree, type_node):
            type_class = 1
        elif pj_is["turn_context"](tree, type_node):
            type_class = 2
        else:
            type_class = 3
    output[0] = type_class

    var id_node: Int64 = -1
    if payload >= 0:
        id_node = session_string_field(tree, payload, StringSlice("id"))
        if id_node < 0:
            id_node = session_string_field(tree, payload, StringSlice("session_id"))
    if id_node < 0:
        id_node = session_string_field(tree, 0, StringSlice("id"))
    if id_node < 0:
        id_node = session_string_field(tree, 0, StringSlice("session_id"))
    output[1] = id_node

    if type_class == 2:
        var model = (
            session_string_field(tree, payload, StringSlice("model"))
            if payload >= 0
            else -1
        )
        if model < 0:
            model = session_string_field(tree, 0, StringSlice("model"))
        output[2] = model

        var effort = (
            session_string_field(tree, payload, StringSlice("effort"))
            if payload >= 0
            else -1
        )
        if effort < 0 and payload >= 0:
            effort = session_string_field(tree, payload, StringSlice("reasoning_effort"))
        if effort < 0:
            effort = session_string_field(tree, 0, StringSlice("effort"))
        if effort < 0:
            effort = session_string_field(tree, 0, StringSlice("reasoning_effort"))
        output[3] = effort

    var thread_name: Int64 = -1
    if payload >= 0:
        thread_name = session_string_field(tree, payload, StringSlice("thread_name"))
        if thread_name < 0:
            thread_name = session_string_field(tree, payload, StringSlice("title"))
        if thread_name < 0 and payload_metadata >= 0:
            thread_name = session_string_field(
                tree, payload_metadata, StringSlice("thread_name")
            )
    if thread_name < 0:
        thread_name = session_string_field(tree, 0, StringSlice("thread_name"))
    if thread_name < 0:
        thread_name = session_string_field(tree, 0, StringSlice("title"))
    if thread_name < 0 and metadata >= 0:
        thread_name = session_string_field(tree, metadata, StringSlice("thread_name"))
    output[4] = thread_name

    var cwd: Int64 = -1
    if payload >= 0:
        cwd = session_string_field(tree, payload, StringSlice("cwd"))
        if cwd < 0 and payload_metadata >= 0:
            cwd = session_string_field(tree, payload_metadata, StringSlice("cwd"))
        if cwd < 0:
            cwd = session_string_field(tree, payload, StringSlice("workdir"))
    if cwd < 0:
        cwd = session_string_field(tree, 0, StringSlice("cwd"))
    if cwd < 0 and metadata >= 0:
        cwd = session_string_field(tree, metadata, StringSlice("cwd"))
    if cwd < 0:
        cwd = session_string_field(tree, 0, StringSlice("workdir"))
    output[5] = cwd

    var updated: Int64 = session_string_field(tree, 0, StringSlice("updated_at"))
    if updated < 0:
        updated = session_string_field(tree, 0, StringSlice("timestamp"))
    if updated < 0 and payload >= 0:
        updated = session_string_field(tree, payload, StringSlice("updated_at"))
    if updated < 0 and payload >= 0:
        updated = session_string_field(tree, payload, StringSlice("timestamp"))
    output[6] = updated

    var parent: Int64 = -1
    if payload >= 0:
        parent = session_nested_string5(
            tree,
            0,
            StringSlice("payload"),
            StringSlice("source"),
            StringSlice("subagent"),
            StringSlice("thread_spawn"),
            StringSlice("parent_thread_id"),
        )
    if parent < 0:
        var source = session_object_field(tree, 0, StringSlice("source"))
        var subagent = (
            session_object_field(tree, source, StringSlice("subagent"))
            if source >= 0
            else -1
        )
        var spawn = (
            session_object_field(tree, subagent, StringSlice("thread_spawn"))
            if subagent >= 0
            else -1
        )
        if spawn >= 0:
            parent = session_string_field(
                tree, spawn, StringSlice("parent_thread_id")
            )
    if parent < 0 and payload >= 0:
        parent = session_string_field(tree, payload, StringSlice("parent_thread_id"))
    if parent < 0:
        parent = session_string_field(tree, 0, StringSlice("parent_thread_id"))
    output[7] = parent

    var provider: Int64 = -1
    if payload >= 0:
        provider = session_string_field(tree, payload, StringSlice("model_provider"))
        if provider < 0 and payload_metadata >= 0:
            provider = session_string_field(
                tree, payload_metadata, StringSlice("model_provider")
            )
    if provider < 0:
        provider = session_string_field(tree, 0, StringSlice("model_provider"))
    if provider < 0 and metadata >= 0:
        provider = session_string_field(tree, metadata, StringSlice("model_provider"))
    output[8] = provider
    return SESSION_REPORT_OK
