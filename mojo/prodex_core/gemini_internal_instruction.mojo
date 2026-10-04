from std.collections import Array
from std.memory import Pointer

from gemini_internal_instruction_catalog import (
    GEMINI_INTERNAL_INSTRUCTION_LEAK_MARKERS_CATALOG,
    GEMINI_INTERNAL_INSTRUCTION_LEAK_PREFIXES_CATALOG,
    GEMINI_INTERNAL_INSTRUCTION_SAFE_STATUS_MARKERS_CATALOG,
)
from rich_text import rich_trim_bounds, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr


comptime GEMINI_INTERNAL_INSTRUCTION_ABI_VERSION: Int64 = 1
comptime GEMINI_INTERNAL_INSTRUCTION_LEAK_TEXT: Int64 = 1
comptime GEMINI_INTERNAL_INSTRUCTION_SANITIZE_TEXT: Int64 = 2
comptime GEMINI_INTERNAL_INSTRUCTION_NORMALIZE_CORPUS: Int64 = 3
comptime GEMINI_INTERNAL_INSTRUCTION_TEXT_ECHO: Int64 = 4


def gemini_instruction_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def gemini_instruction_word_byte(value: UInt8) -> Bool:
    return (
        value >= 48
        and value <= 57
        or value >= 65
        and value <= 90
        or value >= 97
        and value <= 122
        or value == 45
        or value == 95
    )


def gemini_instruction_range_contains(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    needle: StringSlice,
) -> Bool:
    var needle_length = Int64(needle.byte_length())
    if needle_length == 0:
        return True
    if start < 0 or end < start or end - start < needle_length:
        return False
    var source = rich_view_ptr(view)
    var wanted = needle.unsafe_ptr()
    for offset in range(end - start - needle_length + 1):
        var matched = True
        for index in range(needle_length):
            if gemini_instruction_ascii_lower(
                source[unsafe_offset=start + offset + index]
            ) != gemini_instruction_ascii_lower(wanted[unsafe_offset=index]):
                matched = False
                break
        if matched:
            return True
    return False


def gemini_instruction_catalog_matches(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    catalog: StringSlice,
    prefix: Bool,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var catalog_length = Int64(catalog.byte_length())
    if catalog_length == 0:
        return False
    var source = rich_view_ptr(view)
    var patterns = catalog.unsafe_ptr()
    var cursor: Int64 = 0
    while cursor < catalog_length:
        var pattern_start = cursor
        while cursor < catalog_length and patterns[unsafe_offset=cursor] != 10:
            cursor += 1
        var pattern_length = cursor - pattern_start
        if pattern_length > 0 and pattern_length <= end - start:
            var last_offset = end - start - pattern_length
            if prefix:
                last_offset = 0
            for offset in range(last_offset + 1):
                var matched = True
                for index in range(pattern_length):
                    if gemini_instruction_ascii_lower(
                        source[unsafe_offset=start + offset + index]
                    ) != gemini_instruction_ascii_lower(
                        patterns[unsafe_offset=pattern_start + index]
                    ):
                        matched = False
                        break
                if matched:
                    return True
        if cursor < catalog_length:
            cursor += 1
    return False


def gemini_instruction_trim_range(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Array[Int64, 2]:
    var bounds = Array[Int64, 2](fill=0)
    if start < 0 or end < start or end > Int64(view.len):
        return bounds^
    var slice = ProdexRichStringView(view.ptr + UInt(start), UInt(end - start))
    var trimmed = rich_trim_bounds(slice)
    bounds[0] = start + trimmed[0]
    bounds[1] = start + trimmed[1]
    return bounds^


def gemini_instruction_text_is_leak(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    var bounds = gemini_instruction_trim_range(view, start, end)
    var trimmed_start = bounds[0]
    var trimmed_end = bounds[1]
    if trimmed_start >= trimmed_end:
        return False

    if gemini_instruction_catalog_matches(
        view,
        trimmed_start,
        trimmed_end,
        GEMINI_INTERNAL_INSTRUCTION_LEAK_PREFIXES_CATALOG,
        True,
    ):
        return True
    if gemini_instruction_catalog_matches(
        view,
        trimmed_start,
        trimmed_end,
        GEMINI_INTERNAL_INSTRUCTION_LEAK_MARKERS_CATALOG,
        False,
    ):
        return True
    return gemini_instruction_range_contains(
        view,
        trimmed_start,
        trimmed_end,
        StringSlice("all commands run with user privileges"),
    ) and gemini_instruction_range_contains(
        view,
        trimmed_start,
        trimmed_end,
        StringSlice("execute requested tool tasks"),
    )


def gemini_instruction_safe_status_line(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    return gemini_instruction_catalog_matches(
        view,
        start,
        end,
        GEMINI_INTERNAL_INSTRUCTION_SAFE_STATUS_MARKERS_CATALOG,
        False,
    )


def gemini_instruction_append_range(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    output_length: Pointer[mut=True, Int64, MutUntrackedOrigin],
):
    var source = rich_view_ptr(view)
    var destination = output_length[]
    for index in range(end - start):
        output[unsafe_offset=destination + index] = source[
            unsafe_offset=start + index
        ]
    output_length[] = destination + end - start


def gemini_instruction_append_literal(
    value: StringSlice,
    output: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    output_length: Pointer[mut=True, Int64, MutUntrackedOrigin],
):
    var source = value.unsafe_ptr()
    var destination = output_length[]
    var length = Int64(value.byte_length())
    for index in range(length):
        output[unsafe_offset=destination + index] = source[unsafe_offset=index]
    output_length[] = destination + length


def gemini_instruction_sanitize(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    output_length: Pointer[mut=True, Int64, MutUntrackedOrigin],
) -> Int64:
    var input_length = Int64(view.len)
    if not gemini_instruction_text_is_leak(view, 0, input_length):
        gemini_instruction_append_range(
            view, 0, input_length, output, output_length
        )
        return 0

    var source = rich_view_ptr(view)
    var cursor: Int64 = 0
    var retained_paragraphs: Int64 = 0
    while cursor <= input_length:
        var paragraph_end = cursor
        while paragraph_end + 1 < input_length:
            if (
                source[unsafe_offset=paragraph_end] == 10
                and source[unsafe_offset=paragraph_end + 1] == 10
            ):
                break
            paragraph_end += 1
        var has_separator = paragraph_end + 1 < input_length
        if not has_separator:
            paragraph_end = input_length

        var paragraph = gemini_instruction_trim_range(
            view, cursor, paragraph_end
        )
        var paragraph_start = paragraph[0]
        var paragraph_stop = paragraph[1]
        if paragraph_start < paragraph_stop:
            if not gemini_instruction_text_is_leak(
                view, paragraph_start, paragraph_stop
            ):
                if retained_paragraphs > 0:
                    gemini_instruction_append_literal(
                        StringSlice("\n\n"), output, output_length
                    )
                gemini_instruction_append_range(
                    view, paragraph_start, paragraph_stop, output, output_length
                )
                retained_paragraphs += 1
            else:
                var line_cursor = paragraph_start
                var retained_lines: Int64 = 0
                while line_cursor <= paragraph_stop:
                    var line_end = line_cursor
                    while (
                        line_end < paragraph_stop
                        and source[unsafe_offset=line_end] != 10
                    ):
                        line_end += 1
                    var content_end = line_end
                    if (
                        line_end < paragraph_stop
                        and content_end > line_cursor
                        and source[unsafe_offset=content_end - 1] == 13
                    ):
                        content_end -= 1
                    var line = gemini_instruction_trim_range(
                        view, line_cursor, content_end
                    )
                    if line[0] < line[
                        1
                    ] and gemini_instruction_safe_status_line(
                        view, line[0], line[1]
                    ):
                        if retained_lines == 0 and retained_paragraphs > 0:
                            gemini_instruction_append_literal(
                                StringSlice("\n\n"), output, output_length
                            )
                        elif retained_lines > 0:
                            gemini_instruction_append_literal(
                                StringSlice("\n"), output, output_length
                            )
                        gemini_instruction_append_range(
                            view, line[0], line[1], output, output_length
                        )
                        retained_lines += 1
                    if line_end >= paragraph_stop:
                        break
                    line_cursor = line_end + 1
                if retained_lines > 0:
                    retained_paragraphs += 1

        if not has_separator:
            break
        cursor = paragraph_end + 2

    return 1 if retained_paragraphs > 0 else 2


def gemini_instruction_normalize_corpus(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    output_length: Pointer[mut=True, Int64, MutUntrackedOrigin],
):
    var source = rich_view_ptr(view)
    var input_length = Int64(view.len)
    var cursor: Int64 = 0
    while cursor < input_length:
        while cursor < input_length and not gemini_instruction_word_byte(
            source[unsafe_offset=cursor]
        ):
            cursor += 1
        var token_start = cursor
        while cursor < input_length and gemini_instruction_word_byte(
            source[unsafe_offset=cursor]
        ):
            cursor += 1
        var token_length = cursor - token_start
        if token_length >= 2:
            if output_length[] > 0:
                gemini_instruction_append_literal(
                    StringSlice(" "), output, output_length
                )
            var destination = output_length[]
            for index in range(token_length):
                output[
                    unsafe_offset=destination + index
                ] = gemini_instruction_ascii_lower(
                    source[unsafe_offset=token_start + index]
                )
            output_length[] = destination + token_length


def gemini_instruction_corpus_contains_window(
    text: ProdexRichStringView,
    corpus: ProdexRichStringView,
    starts: Array[Int64, 8],
    ends: Array[Int64, 8],
    first_slot: Int64,
) -> Bool:
    var total_length: Int64 = 7
    for index in range(8):
        var slot = (first_slot + Int64(index)) % 8
        total_length += ends.unsafe_get(Int(slot)) - starts.unsafe_get(
            Int(slot)
        )
    if total_length > Int64(corpus.len):
        return False

    var text_ptr = rich_view_ptr(text)
    var corpus_ptr = rich_view_ptr(corpus)
    for corpus_start in range(Int64(corpus.len) - total_length + 1):
        var corpus_cursor = corpus_start
        var matched = True
        for word_index in range(8):
            var slot = (first_slot + Int64(word_index)) % 8
            var word_start = starts.unsafe_get(Int(slot))
            var word_end = ends.unsafe_get(Int(slot))
            var word_length = word_end - word_start
            for offset in range(word_length):
                if (
                    gemini_instruction_ascii_lower(
                        text_ptr[unsafe_offset=word_start + offset]
                    )
                    != corpus_ptr[unsafe_offset=corpus_cursor + offset]
                ):
                    matched = False
                    break
            if not matched:
                break
            corpus_cursor += word_length
            if word_index < 7:
                if corpus_ptr[unsafe_offset=corpus_cursor] != 32:
                    matched = False
                    break
                corpus_cursor += 1
        if matched:
            return True
    return False


def gemini_instruction_text_echoes(
    text: ProdexRichStringView, corpus: ProdexRichStringView
) -> Bool:
    if corpus.len == 0:
        return False
    var source = rich_view_ptr(text)
    var input_length = Int64(text.len)
    var cursor: Int64 = 0
    var token_count: Int64 = 0
    var windows_checked: Int64 = 0
    var starts = Array[Int64, 8](fill=0)
    var ends = Array[Int64, 8](fill=0)
    while cursor <= input_length:
        var token_start = cursor
        while cursor < input_length and gemini_instruction_word_byte(
            source[unsafe_offset=cursor]
        ):
            cursor += 1
        var token_length = cursor - token_start
        if token_length >= 2:
            var slot = token_count % 8
            # Modulo keeps this slot within the eight-element arrays.
            starts.unsafe_get(Int(slot)) = token_start
            ends.unsafe_get(Int(slot)) = cursor
            token_count += 1
            if token_count >= 8:
                if gemini_instruction_corpus_contains_window(
                    text, corpus, starts, ends, token_count % 8
                ):
                    return True
                windows_checked += 1
                if windows_checked >= 128:
                    return False
        cursor += 1
    return False


@export("prodex_mojo_gemini_internal_instruction_v1")
def prodex_mojo_gemini_internal_instruction_v1(
    abi_version: Int64,
    operation: Int64,
    text_address: UInt,
    text_length: Int64,
    corpus_address: UInt,
    corpus_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    decision_address: UInt,
    output_length_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_INTERNAL_INSTRUCTION_ABI_VERSION:
        return 2
    if (
        operation < GEMINI_INTERNAL_INSTRUCTION_LEAK_TEXT
        or operation > GEMINI_INTERNAL_INSTRUCTION_TEXT_ECHO
        or text_length < 0
        or corpus_length < 0
        or output_capacity < 0
        or decision_address == 0
        or output_length_address == 0
        or (text_length > 0 and text_address == 0)
        or (
            operation == GEMINI_INTERNAL_INSTRUCTION_TEXT_ECHO
            and corpus_length > 0
            and corpus_address == 0
        )
    ):
        return 1
    var text = ProdexRichStringView(text_address, UInt(text_length))
    var corpus = ProdexRichStringView(corpus_address, UInt(corpus_length))
    if (
        not rich_view_valid(text, Int64(text.len))
        or operation == GEMINI_INTERNAL_INSTRUCTION_TEXT_ECHO
        and not rich_view_valid(corpus, Int64(corpus.len))
    ):
        return 1
    if (
        operation == GEMINI_INTERNAL_INSTRUCTION_SANITIZE_TEXT
        or operation == GEMINI_INTERNAL_INSTRUCTION_NORMALIZE_CORPUS
    ) and (output_address == 0 or output_capacity < text_length):
        return 3

    var decision = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(decision_address)
    )
    var output_length_result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_length_address)
    )
    decision[] = 0
    output_length_result[] = 0

    if operation == GEMINI_INTERNAL_INSTRUCTION_LEAK_TEXT:
        decision[] = Int64(
            gemini_instruction_text_is_leak(text, 0, text_length)
        )
        return 0
    if operation == GEMINI_INTERNAL_INSTRUCTION_TEXT_ECHO:
        decision[] = Int64(gemini_instruction_text_echoes(text, corpus))
        return 0

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if operation == GEMINI_INTERNAL_INSTRUCTION_SANITIZE_TEXT:
        decision[] = gemini_instruction_sanitize(
            text, output, output_length_result
        )
        return 0

    gemini_instruction_normalize_corpus(text, output, output_length_result)
    return 0
