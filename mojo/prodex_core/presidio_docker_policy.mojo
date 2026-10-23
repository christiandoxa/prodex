# Presidio Docker trust policy. Rust owns process execution and JSON acquisition.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_prefix, rich_view_matches_literal
from parsed_json import (
    JSON_STRING, JSON_ARRAY, JSON_OBJECT, ParsedJson, ParsedJsonNode,
    pj_child, pj_next, pj_kind, pj_field, pj_text, pj_is, pj_equal, pj_literal, pj_valid,
)
from json_sink import JsonSink, js_literal, js_view
from update_notice_policy import update_notice_parse_release_version, UpdateNoticeReleaseVersion


def presidio_image_check(
    image: ProdexRichStringView, service: ProdexRichStringView,
    minimum: UpdateNoticeReleaseVersion,
) -> Tuple[Int64, ProdexRichStringView]:
    var prefix = pj_literal("ghcr.io/data-privacy-stack/presidio-")
    var prefix_length = Int64(prefix.len + service.len + 1)
    var source = rich_view_ptr(image)
    if (
        Int64(image.len) < prefix_length
        or not rich_view_prefix["ghcr.io/data-privacy-stack/presidio-"](image, False)
        or not pj_equal(ProdexRichStringView(image.ptr + prefix.len, service.len), service)
        or source[unsafe_offset=prefix_length - 1] != 58
    ):
        return (Int64(1), image.copy())
    var version_end = prefix_length
    while version_end < Int64(image.len) and source[unsafe_offset=version_end] != 64:
        version_end += 1
    var version = ProdexRichStringView(image.ptr + UInt(prefix_length), UInt(version_end - prefix_length))
    var bounds = rich_trim_bounds(version)
    # Docker tags require exact SemVer; release notices also accept trim and v prefixes.
    if version.len == 0 or bounds[0] != 0 or bounds[1] != Int64(version.len) or rich_view_ptr(version)[] == 118:
        return (Int64(2), version^)
    var parsed = update_notice_parse_release_version(version)
    if not parsed.valid:
        return (Int64(2), version^)
    if parsed.has_pre or (
        parsed.major < minimum.major
        or parsed.major == minimum.major and parsed.minor < minimum.minor
        or parsed.major == minimum.major and parsed.minor == minimum.minor and parsed.patch < minimum.patch
    ):
        return (Int64(3), version^)
    if version_end < Int64(image.len):
        var digest = ProdexRichStringView(image.ptr + UInt(version_end + 1), image.len - UInt(version_end + 1))
        if not rich_view_prefix["sha256:"](digest, False):
            return (Int64(4), version^)
        if digest.len != 71:
            return (Int64(5), version^)
        var bytes = rich_view_ptr(digest)
        for index in range(7, 71):
            var byte = bytes[unsafe_offset=index]
            if not (byte >= 48 and byte <= 57 or byte >= 65 and byte <= 70 or byte >= 97 and byte <= 102):
                return (Int64(5), version^)
    return (Int64(0), version^)


def presidio_image_diagnostic(
    sink: Pointer[mut=True, JsonSink, _], issue: Int64,
    service: ProdexRichStringView, version: ProdexRichStringView,
    minimum: ProdexRichStringView, latest: ProdexRichStringView,
):
    js_literal(sink, StringSlice("Presidio "))
    js_view(sink, service)
    if issue == 1:
        js_literal(sink, StringSlice(" image must use the official ghcr.io/data-privacy-stack/presidio-"))
        js_view(sink, service)
        js_literal(sink, StringSlice(":<version> image"))
    elif issue == 2:
        js_literal(sink, StringSlice(" image has invalid version "))
        js_view(sink, version)
    elif issue == 3:
        js_literal(sink, StringSlice(" "))
        js_view(sink, version)
        js_literal(sink, StringSlice(" is incompatible; Prodex requires "))
        js_view(sink, minimum)
        js_literal(sink, StringSlice(" or newer. Update Presidio to the latest stable release (release-qualified reference: "))
        js_view(sink, latest)
        js_literal(sink, StringSlice(")"))
    elif issue == 4:
        js_literal(sink, StringSlice(" image digest must use sha256"))
    else:
        js_literal(sink, StringSlice(" image digest must be a 64-character SHA-256"))


def presidio_container_valid(
    tree: ParsedJson, container: Int64, name: ProdexRichStringView,
    service: ProdexRichStringView, host_port: ProdexRichStringView,
    minimum: UpdateNoticeReleaseVersion,
) -> Bool:
    var config = pj_field(tree, container, StringSlice("Config"))
    var labels = pj_field(tree, config, StringSlice("Labels"))
    var owner = pj_field(tree, labels, StringSlice("com.prodex.presidio.service"))
    var image = pj_field(tree, config, StringSlice("Image"))
    if (
        not pj_is["true"](tree, pj_field(tree, labels, StringSlice("com.prodex.presidio.managed")))
        or pj_kind(tree, owner) != JSON_STRING or not pj_equal(pj_text(tree, owner), name)
        or pj_kind(tree, image) != JSON_STRING
        or presidio_image_check(pj_text(tree, image), service, minimum)[0] != 0
    ):
        return False
    var host = pj_field(tree, container, StringSlice("HostConfig"))
    var bindings = pj_field(tree, host, StringSlice("PortBindings"))
    if pj_kind(tree, bindings) != JSON_OBJECT:
        return False
    var entry = pj_child(tree, bindings)
    if entry < 0 or pj_next(tree, entry) >= 0 or not pj_equal(tree.nodes[unsafe_offset=entry].key, pj_literal("3000/tcp")) or pj_kind(tree, entry) != JSON_ARRAY:
        return False
    var binding = pj_child(tree, entry)
    if binding < 0 or pj_next(tree, binding) >= 0:
        return False
    var port = pj_field(tree, binding, StringSlice("HostPort"))
    return pj_kind(tree, port) == JSON_STRING and pj_equal(pj_text(tree, port), host_port) and pj_is["127.0.0.1"](tree, pj_field(tree, binding, StringSlice("HostIp")))


@export("prodex_presidio_docker_validation_v1")
def prodex_presidio_docker_validation_v1(
    abi: Int64, operation: Int64, flag: Int64,
    nodes_address: UInt64, count: Int64, raw_address: UInt64, raw_length: Int64,
    scratch_address: UInt64, scratch_count: Int64, measuring: Int64,
    output_address: UInt64, capacity: Int64, metadata_address: UInt64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if (
        operation < 0 or operation > 1 or flag != 0 or count <= 0 or count > 1_048_576
        or nodes_address == 0 or nodes_address % 8 != 0 or raw_length != 0
        or scratch_count < 0 or capacity < 0 or measuring < 0 or measuring > 1
        or metadata_address == 0 or metadata_address % 8 != 0
        or measuring == 1 and (output_address != 0 or capacity != 0)
        or measuring == 0 and output_address == 0
    ):
        return 1
    var tree = ParsedJson(Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)), count, ProdexRichStringView(UInt(raw_address), 0))
    if not pj_valid(tree) or pj_kind(tree, 0) != JSON_OBJECT:
        return 1
    var minimum_index = pj_field(tree, 0, StringSlice("minimum"))
    if pj_kind(tree, minimum_index) != JSON_STRING:
        return 1
    var minimum_text = pj_text(tree, minimum_index)
    var minimum = update_notice_parse_release_version(minimum_text)
    if not minimum.valid or minimum.has_pre or minimum.build_end != minimum.build_start:
        return 1
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, measuring == 1, False)
    var error = False
    if operation == 0:
        var image = pj_field(tree, 0, StringSlice("image"))
        var service = pj_field(tree, 0, StringSlice("service"))
        var latest = pj_field(tree, 0, StringSlice("latest"))
        if pj_kind(tree, image) != JSON_STRING or pj_kind(tree, service) != JSON_STRING or pj_kind(tree, latest) != JSON_STRING:
            return 1
        var check = presidio_image_check(pj_text(tree, image), pj_text(tree, service), minimum)
        error = check[0] != 0
        if error:
            presidio_image_diagnostic(Pointer(to=sink), check[0], pj_text(tree, service), check[1], minimum_text, pj_text(tree, latest))
    else:
        var name_index = pj_field(tree, 0, StringSlice("name"))
        var port_index = pj_field(tree, 0, StringSlice("host_port"))
        if pj_kind(tree, name_index) != JSON_STRING or pj_kind(tree, port_index) != JSON_STRING:
            return 1
        var name = pj_text(tree, name_index)
        var service = ProdexRichStringView(0, 0)
        if rich_view_matches_literal["presidio-analyzer"](name, False):
            service = pj_literal("analyzer")
        elif rich_view_matches_literal["presidio-anonymizer"](name, False):
            service = pj_literal("anonymizer")
        else:
            error = True
            js_literal(Pointer(to=sink), StringSlice("unknown Prodex-managed Presidio container "))
            js_view(Pointer(to=sink), name)
        if not error and not presidio_container_valid(tree, pj_field(tree, 0, StringSlice("container")), name, service, pj_text(tree, port_index), minimum):
            error = True
            js_literal(Pointer(to=sink), StringSlice("refusing existing Presidio container "))
            js_view(Pointer(to=sink), name)
            js_literal(Pointer(to=sink), StringSlice(": Prodex ownership, compatible official image version, or port configuration does not match"))
    if sink.failed:
        return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = Int64(1) if error else Int64(0)
    metadata[unsafe_offset=1] = sink.written
    return 0
