# Artifact line selection and ordered replacement; all storage stays in Rust.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_ptr, rich_view_prefix, rich_codepoint_width
from parsed_json import pj_trim, pj_equal
from json_sink import JsonSink, js_byte, js_view, js_literal

@fieldwise_init
struct ArtifactTextInput(Copyable):
    var operation: Int64
    var text: ProdexRichStringView
    var id: ProdexRichStringView
    var marker: ProdexRichStringView
    var replacement: ProdexRichStringView
    var ranges: UInt64
    var range_count: Int64
    var has_line_range: Int64
    var scratch: UInt64
    var scratch_words: Int64


def at_slice(text: ProdexRichStringView, start: Int64, end: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(text.ptr + UInt(start), UInt(end - start))


def at_short(sink: Pointer[mut=True, JsonSink, _], id: ProdexRichStringView):
    if rich_view_prefix["sc2:"](id, False):
        js_literal(sink, StringSlice("psc2:"))
        js_view(sink, at_slice(id, 4, Int64(id.len)))
    else:
        js_literal(sink, StringSlice("psc:"))
        js_view(sink, at_slice(id, 3, Int64(id.len)) if rich_view_prefix["sc:"](id, False) else id.copy())


def at_prefix_table(pattern: ProdexRichStringView, table: Pointer[mut=True, Int64, _]):
    if pattern.len == 0:
        return
    var source = rich_view_ptr(pattern)
    table[unsafe_offset=0] = 0
    var prefix: Int64 = 0
    for index in range(Int64(1), Int64(pattern.len)):
        while prefix > 0 and source[unsafe_offset=index] != source[unsafe_offset=prefix]:
            prefix = table[unsafe_offset=prefix - 1]
        if source[unsafe_offset=index] == source[unsafe_offset=prefix]:
            prefix += 1
        table[unsafe_offset=index] = prefix


def at_find(text: ProdexRichStringView, pattern: ProdexRichStringView, start: Int64, table: Pointer[mut=True, Int64, _]) -> Int64:
    if pattern.len == 0:
        return start
    var source = rich_view_ptr(text)
    var needle = rich_view_ptr(pattern)
    var matched: Int64 = 0
    for cursor in range(start, Int64(text.len)):
        while matched > 0 and source[unsafe_offset=cursor] != needle[unsafe_offset=matched]:
            matched = table[unsafe_offset=matched - 1]
        if source[unsafe_offset=cursor] == needle[unsafe_offset=matched]:
            matched += 1
        if matched == Int64(pattern.len):
            return cursor + 1 - matched
    return -1


def at_summary(text: ProdexRichStringView, legacy: ProdexRichStringView, short: ProdexRichStringView, table: Pointer[mut=True, Int64, _]) -> Bool:
    var first = pj_trim(text)
    var source = rich_view_ptr(first)
    var end: Int64 = 0
    while end < Int64(first.len) and source[unsafe_offset=end] != 10:
        end += 1
    first = at_slice(first, 0, end)
    var verbose = rich_view_prefix["prodex-sc artifact "](first, False) or rich_view_prefix["prodex-sc repeat "](first, False)
    if verbose:
        at_prefix_table(legacy, table)
        if at_find(first, legacy, 0, table) >= 0:
            return True
    if verbose or rich_view_prefix["psc art "](first, False) or rich_view_prefix["psc rep "](first, False) or rich_view_prefix["psc repeat "](first, False) or rich_view_prefix["psc co "](first, False) or rich_view_prefix["psc cmdout "](first, False):
        at_prefix_table(short, table)
        return at_find(first, short, 0, table) >= 0
    return False


def at_replace(sink: Pointer[mut=True, JsonSink, _], input: ArtifactTextInput, pattern: ProdexRichStringView, table: Pointer[mut=True, Int64, _]):
    var cursor: Int64 = 0
    if pattern.len == 0:
        var source = rich_view_ptr(input.text)
        js_view(sink, input.replacement)
        while cursor < Int64(input.text.len):
            var next = cursor + rich_codepoint_width(source[unsafe_offset=cursor])
            js_view(sink, at_slice(input.text, cursor, next))
            js_view(sink, input.replacement)
            cursor = next
        return
    while cursor < Int64(input.text.len):
        var found = at_find(input.text, pattern, cursor, table)
        if found < 0:
            js_view(sink, at_slice(input.text, cursor, Int64(input.text.len)))
            return
        js_view(sink, at_slice(input.text, cursor, found))
        js_view(sink, input.replacement)
        cursor = found + Int64(pattern.len)


def at_line_spans(input: ArtifactTextInput, spans: Pointer[mut=True, Int64, _]) -> Int64:
    var source = rich_view_ptr(input.text)
    var cursor: Int64 = 0
    var count: Int64 = 0
    while cursor < Int64(input.text.len):
        var start = cursor
        while cursor < Int64(input.text.len) and source[unsafe_offset=cursor] != 10:
            cursor += 1
        var end = cursor
        if cursor < Int64(input.text.len) and end > start and source[unsafe_offset=end - 1] == 13:
            end -= 1
        if count >= input.scratch_words // 2:
            return -1
        spans[unsafe_offset=count * 2] = start
        spans[unsafe_offset=count * 2 + 1] = end
        count += 1
        cursor += 1
    return count


def at_lines(sink: Pointer[mut=True, JsonSink, _], input: ArtifactTextInput, spans: Pointer[mut=True, Int64, _], line_count: Int64):
    var ranges = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(input.ranges))
    for index in range(input.range_count):
        if index > 0:
            js_byte(sink, 10)
        var start = Int64(ranges[unsafe_offset=index * 2]) - 1
        var end = Int64(min(ranges[unsafe_offset=index * 2 + 1], UInt64(line_count)))
        for line in range(start, end):
            if line > start:
                js_byte(sink, 10)
            js_view(sink, at_slice(input.text, spans[unsafe_offset=line * 2], spans[unsafe_offset=line * 2 + 1]))


@export("prodex_smart_context_artifact_text_v1")
def prodex_smart_context_artifact_text_v1(
    abi: Int64, input_address: UInt64, measuring: Int64,
    output: UInt64, capacity: Int64, metadata: UInt64, metadata_count: Int64,
) abi("C") -> Int64:
    if abi != 1:
        return 4
    if input_address == 0 or measuring < 0 or measuring > 1 or capacity < 0 or measuring == 0 and output == 0 or metadata == 0 or metadata_count < 4:
        return 1
    var input = Pointer[mut=False, ArtifactTextInput, ImmUntrackedOrigin](unsafe_from_address=Int(input_address))[].copy()
    if input.operation < 0 or input.operation > 1 or input.range_count < 0 or input.range_count > 1048576 or input.scratch_words < 0 or input.has_line_range < 0 or input.has_line_range > 1:
        return 1
    for text in [input.text.copy(), input.id.copy(), input.marker.copy(), input.replacement.copy()]:
        if not rich_view_valid(text, 67108864):
            return 1
    if input.operation == 0 and (input.range_count > 0 or input.has_line_range == 1):
        if input.ranges == 0:
            return 1
        if input.range_count == 0:
            input.range_count = 1
        elif input.has_line_range == 1:
            input.ranges += 16
    var present = True
    var stop = False
    var direct = False
    var line_count: Int64 = 0
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output)), capacity, 0, True, False)
    if input.operation == 0:
        if input.range_count == 0:
            js_view(Pointer(to=sink), input.text)
            direct = True
        else:
            if input.ranges == 0 or input.scratch == 0:
                return 1
            var spans = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(input.scratch))
            line_count = at_line_spans(input, spans)
            if line_count < 0:
                return 3
            var ranges = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(input.ranges))
            for index in range(input.range_count):
                var start = ranges[unsafe_offset=index * 2]
                var end = ranges[unsafe_offset=index * 2 + 1]
                if start == 0 or end < start:
                    return 1
                present &= start <= UInt64(line_count)
            if present:
                at_lines(Pointer(to=sink), input, spans, line_count)
    else:
        var pattern_bytes = Int64(input.id.len) * 2 + 32
        var pattern_words = (pattern_bytes + 7) // 8
        var table_count = max(Int64(input.marker.len), Int64(input.id.len) + 16)
        if input.scratch == 0 or input.scratch_words < pattern_words + table_count:
            return 1
        var bytes = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(input.scratch))
        var patterns = JsonSink(bytes, pattern_bytes, 0, False, False)
        js_literal(Pointer(to=patterns), StringSlice("prodex-artifact:"))
        js_view(Pointer(to=patterns), input.id)
        var legacy = ProdexRichStringView(UInt(input.scratch), UInt(patterns.written))
        var offset = patterns.written
        at_short(Pointer(to=patterns), input.id)
        var short = ProdexRichStringView(UInt(input.scratch) + UInt(offset), UInt(patterns.written - offset))
        var table = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(input.scratch + UInt64(pattern_words * 8)))
        var chosen = input.marker.copy()
        if input.has_line_range == 0 and at_summary(input.text, legacy, short, table):
            stop = True
            direct = True
        else:
            present = False
            for pattern in [input.marker.copy(), legacy.copy(), short.copy()]:
                at_prefix_table(pattern, table)
                if at_find(input.text, pattern, 0, table) >= 0:
                    chosen = pattern.copy()
                    present = True
                    direct = pattern.len == input.text.len
                    break
            if not present and pj_equal(pj_trim(input.text), input.id):
                present = True
                direct = True
        if direct:
            js_view(Pointer(to=sink), input.replacement)
        elif present:
            at_prefix_table(chosen, table)
            at_replace(Pointer(to=sink), input, chosen, table)
        if sink.failed or measuring == 0 and sink.written > capacity:
            return 3
        if measuring == 0 and present:
            sink.written = 0
            sink.measuring = False
            if direct:
                js_view(Pointer(to=sink), input.replacement)
            else:
                at_replace(Pointer(to=sink), input, chosen, table)
    if sink.failed or measuring == 0 and sink.written > capacity:
        return 3
    if measuring == 0 and input.operation != 1 and present:
        sink.written = 0
        sink.measuring = False
        if direct:
            js_view(Pointer(to=sink), input.text)
        else:
            var spans = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(input.scratch))
            at_lines(Pointer(to=sink), input, spans, line_count)
    var meta = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata))
    meta[unsafe_offset=0] = Int64(present)
    meta[unsafe_offset=1] = sink.written
    meta[unsafe_offset=2] = Int64(stop)
    # Identity plans let the host materialize an existing validated UTF-8 source.
    meta[unsafe_offset=3] = Int64((1 if input.operation == 0 else 2) if direct and present else 0)
    return 0
