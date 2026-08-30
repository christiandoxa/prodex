from std.memory import Pointer
from std.sys.info import align_of, size_of


comptime CONTEXT_SIGNAL_COUNTER_COUNT: Int64 = 7
comptime CONTEXT_SIGNAL_ROW_WIDTH: Int64 = 8
comptime CONTEXT_SIGNAL_MAX_LINES: Int64 = 65_536
comptime CONTEXT_SIGNAL_MAX_KEYS: Int64 = 65_536
comptime CONTEXT_TEXT_ABI_VERSION: Int64 = 1
comptime CONTEXT_TEXT_SOURCE_AFTER: Int64 = 0
comptime CONTEXT_TEXT_SOURCE_BEFORE: Int64 = 1
comptime CONTEXT_TEXT_HASH_SEED: UInt64 = 1469598103934665603


@fieldwise_init
struct ProdexStringView(Copyable):
    var ptr: Optional[Pointer[mut=False, UInt8, ImmUntrackedOrigin]]
    var len: UInt


@fieldwise_init
struct ProdexBytesView(Copyable):
    var ptr: Optional[Pointer[mut=False, UInt8, ImmUntrackedOrigin]]
    var len: UInt


@fieldwise_init
struct ContextTextRowsResult(Copyable):
    var abi_version: Int64
    var before_line_count: Int64
    var after_line_count: Int64
    var before_rows_written: Int64
    var key_count: Int64
    var after_signal_line_count: Int64
    var required_before_rows: Int64
    var required_key_capacity: Int64
    var required_hash_capacity: Int64


def context_text_is_utf8_continuation(value: UInt8) -> Bool:
    return value >= 0x80 and value <= 0xBF


def context_text_is_valid_utf8(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> Bool:
    var index: Int64 = 0
    while index < length:
        var lead = ptr[unsafe_offset=index]
        if lead <= 0x7F:
            index += 1
        elif lead >= 0xC2 and lead <= 0xDF:
            if index + 1 >= length or not context_text_is_utf8_continuation(
                ptr[unsafe_offset=index + 1]
            ):
                return False
            index += 2
        elif lead == 0xE0:
            if index + 2 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if (
                second < 0xA0
                or second > 0xBF
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
            ):
                return False
            index += 3
        elif (lead >= 0xE1 and lead <= 0xEC) or (lead >= 0xEE and lead <= 0xEF):
            if (
                index + 2 >= length
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 1]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
            ):
                return False
            index += 3
        elif lead == 0xED:
            if index + 2 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if (
                second < 0x80
                or second > 0x9F
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
            ):
                return False
            index += 3
        elif lead == 0xF0:
            if index + 3 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if (
                second < 0x90
                or second > 0xBF
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 3]
                )
            ):
                return False
            index += 4
        elif lead >= 0xF1 and lead <= 0xF3:
            if (
                index + 3 >= length
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 1]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 3]
                )
            ):
                return False
            index += 4
        elif lead == 0xF4:
            if index + 3 >= length:
                return False
            var second = ptr[unsafe_offset=index + 1]
            if (
                second < 0x80
                or second > 0x8F
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 2]
                )
                or not context_text_is_utf8_continuation(
                    ptr[unsafe_offset=index + 3]
                )
            ):
                return False
            index += 4
        else:
            return False
    return True


def context_text_view_is_valid(view: ProdexStringView) -> Bool:
    if view.len == 0:
        return True
    if not view.ptr or view.len > UInt(9223372036854775807):
        return False
    return context_text_is_valid_utf8(view.ptr.unsafe_value(), Int64(view.len))


def context_text_hash(view: ProdexStringView) -> UInt64:
    var value = CONTEXT_TEXT_HASH_SEED ^ UInt64(view.len)
    if view.len == 0:
        return value
    ref ptr = view.ptr.unsafe_value()
    var text = StringSlice(
        unsafe_from_utf8=Span[UInt8](unsafe_ptr=ptr, length=Int(view.len))
    )
    var text_ptr = text.unsafe_ptr()
    for index in range(Int64(text.byte_length())):
        value = value ^ UInt64(text_ptr[unsafe_offset=index])
        value = (value << 7) | (value >> 57)
        value = value ^ (value >> 17)
    return value


def context_text_views_equal(
    left: ProdexStringView, right: ProdexStringView
) -> Bool:
    if left.len != right.len:
        return False
    if left.len == 0:
        return True
    ref left_ptr = left.ptr.unsafe_value()
    ref right_ptr = right.ptr.unsafe_value()
    var left_text = StringSlice(
        unsafe_from_utf8=Span[UInt8](unsafe_ptr=left_ptr, length=Int(left.len))
    )
    var right_text = StringSlice(
        unsafe_from_utf8=Span[UInt8](
            unsafe_ptr=right_ptr, length=Int(right.len)
        )
    )
    return left_text == right_text


def context_text_key_matches(
    view: ProdexStringView,
    key_id: Int64,
    before_views: Pointer[mut=False, ProdexStringView, _],
    after_views: Pointer[mut=False, ProdexStringView, _],
    key_sources: Pointer[mut=False, Int64, _],
    key_indices: Pointer[mut=False, Int64, _],
) -> Bool:
    var source = key_sources[unsafe_offset=key_id]
    var index = key_indices[unsafe_offset=key_id]
    if source == CONTEXT_TEXT_SOURCE_AFTER:
        return context_text_views_equal(view, after_views[unsafe_offset=index])
    return context_text_views_equal(view, before_views[unsafe_offset=index])


def context_text_intern(
    view: ProdexStringView,
    source: Int64,
    source_index: Int64,
    before_views: Pointer[mut=False, ProdexStringView, _],
    after_views: Pointer[mut=False, ProdexStringView, _],
    hash_slots: Pointer[mut=True, Int64, _],
    key_hashes: Pointer[mut=True, UInt64, _],
    key_sources: Pointer[mut=True, Int64, _],
    key_indices: Pointer[mut=True, Int64, _],
    after_available: Pointer[mut=True, Int64, _],
    key_count: Pointer[mut=True, Int64, _],
    key_capacity: Int64,
    hash_capacity: Int64,
) -> Int64:
    var hash = context_text_hash(view)
    var slot = Int64(hash % UInt64(hash_capacity))
    for _ in range(hash_capacity):
        var key_id = hash_slots[unsafe_offset=slot]
        if key_id < 0:
            key_id = key_count[]
            if key_id >= key_capacity:
                return -1
            hash_slots[unsafe_offset=slot] = key_id
            key_hashes[unsafe_offset=key_id] = hash
            key_sources[unsafe_offset=key_id] = source
            key_indices[unsafe_offset=key_id] = source_index
            after_available[unsafe_offset=key_id] = 0
            key_count[] = key_id + 1
            return key_id
        if key_hashes[
            unsafe_offset=key_id
        ] == hash and context_text_key_matches(
            view,
            key_id,
            before_views,
            after_views,
            key_sources,
            key_indices,
        ):
            return key_id
        slot += 1
        if slot == hash_capacity:
            slot = 0
    return -1


def context_text_required_hash_capacity(key_capacity: Int64) -> Int64:
    var required: Int64 = 1
    var target = key_capacity * 2
    while required < target:
        required *= 2
    return required


comptime CONTEXT_METADATA_MAX_BYTES: Int64 = 16_384
comptime CONTEXT_METADATA_NO_KIND: Int64 = -1


def context_metadata_token_byte(value: UInt8) -> Bool:
    return (
        value >= 48
        and value <= 57
        or value >= 65
        and value <= 90
        or value >= 97
        and value <= 122
        or value == 45
        or value == 95
        or value == 46
        or value == 47
        or value == 43
    )


def context_metadata_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def context_metadata_next_token(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, cursor: Int64
) -> InlineArray[Int64, 3]:
    var bounds = InlineArray[Int64, 3](fill=-1)
    var index = cursor
    while index < length and not context_metadata_token_byte(
        ptr[unsafe_offset=index]
    ):
        index += 1
    if index >= length:
        return bounds^
    bounds[0] = index
    while index < length and context_metadata_token_byte(
        ptr[unsafe_offset=index]
    ):
        index += 1
    bounds[1] = index
    bounds[2] = index
    return bounds^


def context_metadata_command_bounds(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var bounds = InlineArray[Int64, 2](fill=0)
    var command_start = start
    var index = end
    while index > start:
        index -= 1
        if ptr[unsafe_offset=index] == 47:
            command_start = index + 1
            break
    var command_end = end
    if command_end - command_start >= 4:
        var suffix = command_end - 4
        if (
            context_metadata_lower(ptr[unsafe_offset=suffix]) == 46
            and context_metadata_lower(ptr[unsafe_offset=suffix + 1]) == 101
            and context_metadata_lower(ptr[unsafe_offset=suffix + 2]) == 120
            and context_metadata_lower(ptr[unsafe_offset=suffix + 3]) == 101
        ):
            command_end = suffix
    bounds[0] = command_start
    bounds[1] = command_end
    return bounds^


def context_metadata_token_equals[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_metadata_command_bounds(ptr, start, end)
    var length = bounds[1] - bounds[0]
    if length != Int64(literal.byte_length()):
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if (
            context_metadata_lower(ptr[unsafe_offset=bounds[0] + index])
            != expected[unsafe_offset=index]
        ):
            return False
    return True


def context_metadata_token_ends_with[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_metadata_command_bounds(ptr, start, end)
    var suffix_length = Int64(literal.byte_length())
    var length = bounds[1] - bounds[0]
    if length < suffix_length:
        return False
    var expected = literal.unsafe_ptr()
    var offset = bounds[1] - suffix_length
    for index in range(suffix_length):
        if (
            context_metadata_lower(ptr[unsafe_offset=offset + index])
            != expected[unsafe_offset=index]
        ):
            return False
    return True


def context_metadata_token_contains[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_metadata_command_bounds(ptr, start, end)
    var needle_length = Int64(literal.byte_length())
    var length = bounds[1] - bounds[0]
    if needle_length == 0:
        return True
    if length < needle_length:
        return False
    var expected = literal.unsafe_ptr()
    for offset in range(length - needle_length + 1):
        var matched = True
        for index in range(needle_length):
            if (
                context_metadata_lower(ptr[unsafe_offset=bounds[0] + offset + index])
                != expected[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def context_metadata_token_is_option_or_shell_glue(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var bounds = context_metadata_command_bounds(ptr, start, end)
    if bounds[1] <= bounds[0]:
        return True
    var first = context_metadata_lower(ptr[unsafe_offset=bounds[0]])
    if first == 45 or first == 43:
        return True
    return (
        context_metadata_token_equals["cmd"](ptr, start, end)
        or context_metadata_token_equals["command"](ptr, start, end)
        or context_metadata_token_equals["args"](ptr, start, end)
        or context_metadata_token_equals["arguments"](ptr, start, end)
        or context_metadata_token_equals["metadata"](ptr, start, end)
        or context_metadata_token_equals["name"](ptr, start, end)
        or context_metadata_token_equals["tool"](ptr, start, end)
        or context_metadata_token_equals["tool_name"](ptr, start, end)
        or context_metadata_token_equals["shell"](ptr, start, end)
        or context_metadata_token_equals["bash"](ptr, start, end)
        or context_metadata_token_equals["sh"](ptr, start, end)
        or context_metadata_token_equals["zsh"](ptr, start, end)
        or context_metadata_token_equals["fish"](ptr, start, end)
        or context_metadata_token_equals["powershell"](ptr, start, end)
        or context_metadata_token_equals["pwsh"](ptr, start, end)
        or context_metadata_token_equals["python"](ptr, start, end)
        or context_metadata_token_equals["python3"](ptr, start, end)
        or context_metadata_token_equals["py"](ptr, start, end)
        or context_metadata_token_equals["node"](ptr, start, end)
        or context_metadata_token_equals["npx"](ptr, start, end)
        or context_metadata_token_equals["bunx"](ptr, start, end)
        or context_metadata_token_equals["uv"](ptr, start, end)
        or context_metadata_token_equals["uvx"](ptr, start, end)
        or context_metadata_token_equals["poetry"](ptr, start, end)
        or context_metadata_token_equals["pipenv"](ptr, start, end)
        or context_metadata_token_equals["exec_command"](ptr, start, end)
        or context_metadata_token_equals["function_call"](ptr, start, end)
        or context_metadata_token_equals["function_call_output"](ptr, start, end)
        or context_metadata_token_equals["shell_call"](ptr, start, end)
        or context_metadata_token_equals["shell_call_output"](ptr, start, end)
        or context_metadata_token_equals["true"](ptr, start, end)
        or context_metadata_token_equals["false"](ptr, start, end)
        or context_metadata_token_equals["null"](ptr, start, end)
    )


def context_metadata_token_option_takes_value(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        context_metadata_token_equals["-c"](ptr, start, end)
        or context_metadata_token_equals["-m"](ptr, start, end)
        or context_metadata_token_equals["-p"](ptr, start, end)
        or context_metadata_token_equals["--config"](ptr, start, end)
        or context_metadata_token_equals["--git-dir"](ptr, start, end)
        or context_metadata_token_equals["--work-tree"](ptr, start, end)
        or context_metadata_token_equals["--manifest-path"](ptr, start, end)
        or context_metadata_token_equals["--package"](ptr, start, end)
        or context_metadata_token_equals["--bin"](ptr, start, end)
        or context_metadata_token_equals["--example"](ptr, start, end)
        or context_metadata_token_equals["--target"](ptr, start, end)
        or context_metadata_token_equals["--project"](ptr, start, end)
        or context_metadata_token_equals["--cwd"](ptr, start, end)
        or context_metadata_token_equals["--prefix"](ptr, start, end)
        or context_metadata_token_equals["--directory"](ptr, start, end)
    )


def context_metadata_subcommand_after(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, cursor: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var scan = cursor
    var skip_next = False
    while scan < length:
        var token = context_metadata_next_token(ptr, length, scan)
        if token[0] < 0:
            break
        scan = token[2]
        if skip_next:
            skip_next = False
            continue
        if context_metadata_token_option_takes_value(ptr, token[0], token[1]):
            skip_next = True
            continue
        if not context_metadata_token_is_option_or_shell_glue(
            ptr, token[0], token[1]
        ):
            result[0] = token[0]
            result[1] = token[1]
            return result^
    return result^


def context_metadata_package_script_after(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, cursor: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var scan = cursor
    var saw_run = False
    var skip_next = False
    while scan < length:
        var token = context_metadata_next_token(ptr, length, scan)
        if token[0] < 0:
            break
        scan = token[2]
        if skip_next:
            skip_next = False
            continue
        if context_metadata_token_option_takes_value(ptr, token[0], token[1]):
            skip_next = True
            continue
        if context_metadata_token_is_option_or_shell_glue(
            ptr, token[0], token[1]
        ):
            continue
        if context_metadata_token_equals["run"](ptr, token[0], token[1]) or context_metadata_token_equals[
            "run-script"
        ](ptr, token[0], token[1]):
            saw_run = True
            continue
        if (
            context_metadata_token_equals["test"](ptr, token[0], token[1])
            or context_metadata_token_equals["t"](ptr, token[0], token[1])
            or context_metadata_token_equals["typecheck"](ptr, token[0], token[1])
            or context_metadata_token_equals["type-check"](ptr, token[0], token[1])
            or context_metadata_token_equals["tsc"](ptr, token[0], token[1])
            or context_metadata_token_equals["check"](ptr, token[0], token[1])
            or saw_run
            and (
                context_metadata_token_contains["test"](ptr, token[0], token[1])
                or context_metadata_token_contains["typecheck"](ptr, token[0], token[1])
            )
        ):
            result[0] = token[0]
            result[1] = token[1]
            return result^
        return result^
    return result^


def context_metadata_package_install_after(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, cursor: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var scan = cursor
    var skip_next = False
    while scan < length:
        var token = context_metadata_next_token(ptr, length, scan)
        if token[0] < 0:
            break
        scan = token[2]
        if skip_next:
            skip_next = False
            continue
        if context_metadata_token_option_takes_value(ptr, token[0], token[1]):
            skip_next = True
            continue
        if context_metadata_token_is_option_or_shell_glue(
            ptr, token[0], token[1]
        ):
            continue
        if (
            context_metadata_token_equals["install"](ptr, token[0], token[1])
            or context_metadata_token_equals["i"](ptr, token[0], token[1])
            or context_metadata_token_equals["ci"](ptr, token[0], token[1])
            or context_metadata_token_equals["add"](ptr, token[0], token[1])
            or context_metadata_token_equals["update"](ptr, token[0], token[1])
            or context_metadata_token_equals["upgrade"](ptr, token[0], token[1])
            or context_metadata_token_equals["sync"](ptr, token[0], token[1])
        ):
            result[0] = token[0]
            result[1] = token[1]
            return result^
        return result^
    return result^


def context_metadata_direct_kind(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if (
        context_metadata_token_equals["rg"](ptr, start, end)
        or context_metadata_token_equals["ripgrep"](ptr, start, end)
        or context_metadata_token_equals["grep"](ptr, start, end)
        or context_metadata_token_equals["egrep"](ptr, start, end)
        or context_metadata_token_equals["fgrep"](ptr, start, end)
    ):
        return 6
    if (
        context_metadata_token_equals["ls"](ptr, start, end)
        or context_metadata_token_equals["find"](ptr, start, end)
        or context_metadata_token_equals["tree"](ptr, start, end)
    ):
        return 7
    if (
        context_metadata_token_equals["pytest"](ptr, start, end)
        or context_metadata_token_equals["py.test"](ptr, start, end)
        or context_metadata_token_equals["tsc"](ptr, start, end)
        or context_metadata_token_equals["ruff"](ptr, start, end)
        or context_metadata_token_equals["mypy"](ptr, start, end)
        or context_metadata_token_equals["biome"](ptr, start, end)
        or context_metadata_token_equals["oxlint"](ptr, start, end)
        or context_metadata_token_equals["eslint"](ptr, start, end)
        or context_metadata_token_equals["playwright"](ptr, start, end)
        or context_metadata_token_equals["cypress"](ptr, start, end)
        or context_metadata_token_ends_with["-tsc"](ptr, start, end)
        or context_metadata_token_ends_with["_tsc"](ptr, start, end)
    ):
        return 4
    if (
        context_metadata_token_equals["bazel"](ptr, start, end)
        or context_metadata_token_equals["bazelisk"](ptr, start, end)
        or context_metadata_token_equals["nx"](ptr, start, end)
        or context_metadata_token_equals["turbo"](ptr, start, end)
        or context_metadata_token_equals["pip"](ptr, start, end)
        or context_metadata_token_equals["pip3"](ptr, start, end)
        or context_metadata_token_equals["uv"](ptr, start, end)
        or context_metadata_token_equals["nyc"](ptr, start, end)
        or context_metadata_token_equals["c8"](ptr, start, end)
        or context_metadata_token_equals["vite"](ptr, start, end)
        or context_metadata_token_equals["next"](ptr, start, end)
        or context_metadata_token_equals["docker-compose"](ptr, start, end)
    ):
        return 9
    return -1


def context_metadata_kind_for_token(
    ptr: Pointer[mut=False, UInt8, _],
    length: Int64,
    start: Int64,
    end: Int64,
    next_cursor: Int64,
) -> Int64:
    var direct = context_metadata_direct_kind(ptr, start, end)
    if direct >= 0:
        return direct

    var subcommand = context_metadata_subcommand_after(
        ptr, length, next_cursor
    )
    if (
        context_metadata_token_equals["gradle"](ptr, start, end)
        or context_metadata_token_equals["gradlew"](ptr, start, end)
    ) and subcommand[0] >= 0 and (
        context_metadata_token_equals["test"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["check"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["build"](ptr, subcommand[0], subcommand[1])
    ):
        return 9
    if (
        context_metadata_token_equals["mvn"](ptr, start, end)
        or context_metadata_token_equals["mvnw"](ptr, start, end)
    ) and subcommand[0] >= 0 and (
        context_metadata_token_equals["test"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["verify"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["package"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["install"](ptr, subcommand[0], subcommand[1])
    ):
        return 9
    if (
        context_metadata_token_equals["journalctl"](ptr, start, end)
        or context_metadata_token_equals["tail"](ptr, start, end)
        or context_metadata_token_equals["kubectl"](ptr, start, end)
        and subcommand[0] >= 0
        and context_metadata_token_equals["logs"](ptr, subcommand[0], subcommand[1])
    ):
        return 8
    if context_metadata_token_equals["go"](ptr, start, end) and subcommand[0] >= 0 and (
        context_metadata_token_equals["vet"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["test"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["build"](ptr, subcommand[0], subcommand[1])
    ):
        return 4
    if context_metadata_token_equals["cargo"](ptr, start, end) and subcommand[0] >= 0:
        if (
            context_metadata_token_equals["test"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["check"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["clippy"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["build"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["doc"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["nextest"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["fmt"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["fix"](ptr, subcommand[0], subcommand[1])
        ):
            return 3
        if (
            context_metadata_token_equals["update"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["install"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["fetch"](ptr, subcommand[0], subcommand[1])
        ):
            return 9
    if context_metadata_token_equals["git"](ptr, start, end) and subcommand[0] >= 0:
        if context_metadata_token_equals["status"](ptr, subcommand[0], subcommand[1]):
            return 1
        if (
            context_metadata_token_equals["diff"](ptr, subcommand[0], subcommand[1])
            or context_metadata_token_equals["show"](ptr, subcommand[0], subcommand[1])
        ):
            return 2
        if context_metadata_token_equals["log"](ptr, subcommand[0], subcommand[1]):
            return 5
        if context_metadata_token_equals["grep"](ptr, subcommand[0], subcommand[1]):
            return 6
        if context_metadata_token_equals["ls-files"](ptr, subcommand[0], subcommand[1]):
            return 7
    if context_metadata_token_equals["docker"](ptr, start, end) and subcommand[0] >= 0 and (
        context_metadata_token_equals["compose"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["build"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["buildx"](ptr, subcommand[0], subcommand[1])
        or context_metadata_token_equals["pull"](ptr, subcommand[0], subcommand[1])
    ):
        return 9
    if (
        context_metadata_token_equals["npm"](ptr, start, end)
        or context_metadata_token_equals["pnpm"](ptr, start, end)
        or context_metadata_token_equals["yarn"](ptr, start, end)
        or context_metadata_token_equals["bun"](ptr, start, end)
    ):
        var script = context_metadata_package_script_after(
            ptr, length, next_cursor
        )
        if script[0] >= 0:
            return 4
        var install = context_metadata_package_install_after(
            ptr, length, next_cursor
        )
        if install[0] >= 0:
            return 9
    return -1


def context_metadata_kind(view: ProdexStringView) -> Int64:
    var ptr = view.ptr.unsafe_value()
    var length = Int64(view.len)
    var cursor: Int64 = 0
    while cursor < length:
        var token = context_metadata_next_token(ptr, length, cursor)
        if token[0] < 0:
            break
        var kind = context_metadata_kind_for_token(
            ptr, length, token[0], token[1], token[2]
        )
        if kind >= 0:
            return kind
        cursor = token[2]
    return CONTEXT_METADATA_NO_KIND


comptime CONTEXT_CI_RESULT_WIDTH: Int64 = 7
comptime CONTEXT_CI_MARKER: Int64 = 1
comptime CONTEXT_CI_ANNOTATION: Int64 = 2
comptime CONTEXT_CI_JOB: Int64 = 4
comptime CONTEXT_CI_STEP: Int64 = 8
comptime CONTEXT_CI_EXIT_CODE: Int64 = 16
comptime CONTEXT_CI_FAILURE_TEXT: Int64 = 32


def context_text_codepoint_width(value: UInt8) -> Int64:
    if value <= 0x7F:
        return 1
    if value <= 0xDF:
        return 2
    if value <= 0xEF:
        return 3
    return 4


def context_text_whitespace_width(
    ptr: Pointer[mut=False, UInt8, _], index: Int64, end: Int64
) -> Int64:
    if index >= end:
        return 0
    var value = ptr[unsafe_offset=index]
    if (
        value == 9
        or value == 10
        or value == 11
        or value == 12
        or value == 13
        or value == 32
    ):
        return 1
    if value == 0xC2 and index + 1 < end:
        var second = ptr[unsafe_offset=index + 1]
        if second == 0x85 or second == 0xA0:
            return 2
    if value == 0xE1 and index + 2 < end:
        if (
            ptr[unsafe_offset=index + 1] == 0x9A
            and ptr[unsafe_offset=index + 2] == 0x80
        ):
            return 3
    if value == 0xE2 and index + 2 < end:
        var second = ptr[unsafe_offset=index + 1]
        var third = ptr[unsafe_offset=index + 2]
        if second == 0x80 and (third >= 0x80 and third <= 0x8A or third >= 0xA8 and third <= 0xA9 or third == 0xAF):
            return 3
        if second == 0x81 and third == 0x9F:
            return 3
    if (
        value == 0xE3
        and index + 2 < end
        and ptr[unsafe_offset=index + 1] == 0x80
        and ptr[unsafe_offset=index + 2] == 0x80
    ):
        return 3
    return 0


def context_text_trim_bounds(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var bounds = InlineArray[Int64, 2](fill=0)
    var first = end
    var last = start
    var index = start
    while index < end:
        var whitespace = context_text_whitespace_width(ptr, index, end)
        if whitespace > 0:
            index += whitespace
            continue
        if first == end:
            first = index
        index += context_text_codepoint_width(ptr[unsafe_offset=index])
        last = index
    if first == end:
        bounds[0] = end
        bounds[1] = end
    else:
        bounds[0] = first
        bounds[1] = last
    return bounds^


def context_text_ascii_starts_at[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or start + length > end:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if (
            context_metadata_lower(ptr[unsafe_offset=start + index])
            != expected[unsafe_offset=index]
        ):
            return False
    return True


def context_text_ascii_starts_exact[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or start + length > end:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if ptr[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def context_text_ascii_contains[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if length == 0:
        return True
    if start < 0 or end < start or start + length > end:
        return False
    var expected = literal.unsafe_ptr()
    for offset in range(start, end - length + 1):
        var matched = True
        for index in range(length):
            if (
                context_metadata_lower(ptr[unsafe_offset=offset + index])
                != expected[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def context_text_ascii_find[literal: StaticString](
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var length = Int64(literal.byte_length())
    if length == 0:
        return start
    if start < 0 or end < start or start + length > end:
        return -1
    var expected = literal.unsafe_ptr()
    for offset in range(start, end - length + 1):
        var matched = True
        for index in range(length):
            if (
                context_metadata_lower(ptr[unsafe_offset=offset + index])
                != expected[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return offset
    return -1


def context_ci_first_integer_token(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var index = start
    var token_start: Int64 = -1
    var started = False
    var has_digit = False
    while index < end:
        var value = ptr[unsafe_offset=index]
        if value >= 48 and value <= 57 or not started and value == 45:
            if not started:
                token_start = index
                started = True
            if value >= 48 and value <= 57:
                has_digit = True
            index += 1
        elif started:
            break
        else:
            index += 1
    if has_digit:
        result[0] = token_start
        result[1] = index
    return result^


def context_ci_exit_code_span(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    var marker = context_text_ascii_find["exit code"](ptr, 0, length)
    if marker >= 0:
        result = context_ci_first_integer_token(ptr, marker + 9, length)
        if result[0] >= 0:
            return result^
    marker = context_text_ascii_find["exit status"](ptr, 0, length)
    if marker >= 0:
        result = context_ci_first_integer_token(ptr, marker + 11, length)
        if result[0] >= 0:
            return result^
    marker = context_text_ascii_find["exited with code"](ptr, 0, length)
    if marker >= 0:
        result = context_ci_first_integer_token(ptr, marker + 16, length)
        if result[0] >= 0:
            return result^
    marker = context_text_ascii_find["failed with code"](ptr, 0, length)
    if marker >= 0:
        result = context_ci_first_integer_token(ptr, marker + 16, length)
        if result[0] >= 0:
            return result^
    marker = context_text_ascii_find["code"](ptr, 0, length)
    if marker >= 0:
        return context_ci_first_integer_token(ptr, marker + 4, length)^
    return result^


def context_ci_line_semantics(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, output: Pointer[mut=True, Int64, _]
) -> None:
    var trimmed = context_text_trim_bounds(ptr, 0, length)
    var start = trimmed[0]
    var end = trimmed[1]
    if start >= end:
        return

    if (
        context_text_ascii_starts_at["##[group]"](ptr, start, end)
        or context_text_ascii_starts_at["##[endgroup]"](ptr, start, end)
        or context_text_ascii_starts_at["##[error]"](ptr, start, end)
        or context_text_ascii_starts_at["::error"](ptr, start, end)
        or context_text_ascii_starts_at["current runner version:"](ptr, start, end)
        or context_text_ascii_starts_at["runner name:"](ptr, start, end)
        or context_text_ascii_starts_at["runner os:"](ptr, start, end)
        or context_text_ascii_starts_at["prepare workflow directory"](ptr, start, end)
        or context_text_ascii_starts_at["prepare all required actions"](ptr, start, end)
        or context_text_ascii_starts_at["complete job"](ptr, start, end)
        or context_text_ascii_starts_at["set up job"](ptr, start, end)
        or context_text_ascii_contains["actions/checkout"](ptr, start, end)
        or context_text_ascii_contains["/_actions/"](ptr, start, end)
        or context_text_ascii_contains["github actions"](ptr, start, end)
        or context_text_ascii_contains["process completed with exit code"](ptr, start, end)
    ):
        output[unsafe_offset=0] |= CONTEXT_CI_MARKER
    if (
        context_text_ascii_starts_at["##[error]"](ptr, start, end)
        or context_text_ascii_starts_at["::error"](ptr, start, end)
    ):
        output[unsafe_offset=0] |= CONTEXT_CI_ANNOTATION
    if (
        context_text_ascii_contains["process completed with exit code"](ptr, start, end)
        or context_text_ascii_contains["failed with exit code"](ptr, start, end)
        or context_text_ascii_contains["exited with code"](ptr, start, end)
        or context_text_ascii_contains["exit status"](ptr, start, end)
    ):
        output[unsafe_offset=0] |= CONTEXT_CI_FAILURE_TEXT

    var job_prefix: Int64 = -1
    if context_text_ascii_starts_at["job:"](ptr, start, end):
        job_prefix = 4
    elif context_text_ascii_starts_at["job name:"](ptr, start, end):
        job_prefix = 9
    elif context_text_ascii_starts_at["workflow job:"](ptr, start, end):
        job_prefix = 13
    elif context_text_ascii_starts_at["failed job:"](ptr, start, end):
        job_prefix = 11
    if job_prefix >= 0:
        var job = context_text_trim_bounds(ptr, start + job_prefix, end)
        if job[0] < job[1]:
            output[unsafe_offset=0] |= CONTEXT_CI_JOB
            output[unsafe_offset=1] = job[0]
            output[unsafe_offset=2] = job[1]
    elif context_text_ascii_starts_at["job "](ptr, start, end) and context_text_ascii_contains[" failed"](ptr, start, end):
        output[unsafe_offset=0] |= CONTEXT_CI_JOB
        output[unsafe_offset=1] = start
        output[unsafe_offset=2] = end

    var body_start = start
    if context_text_ascii_starts_exact["##[group]"](ptr, body_start, end):
        body_start += 9
    var body = context_text_trim_bounds(ptr, body_start, end)
    var step_prefix: Int64 = -1
    if context_text_ascii_starts_at["run "](ptr, body[0], body[1]):
        step_prefix = 4
    elif context_text_ascii_starts_at["step:"](ptr, body[0], body[1]):
        step_prefix = 5
    elif context_text_ascii_starts_at["failed step:"](ptr, body[0], body[1]):
        step_prefix = 12
    if step_prefix >= 0:
        var step = context_text_trim_bounds(ptr, body[0] + step_prefix, body[1])
        if step[0] < step[1]:
            output[unsafe_offset=0] |= CONTEXT_CI_STEP
            output[unsafe_offset=3] = step[0]
            output[unsafe_offset=4] = step[1]

    var exit_code = context_ci_exit_code_span(ptr, length)
    if exit_code[0] >= 0:
        output[unsafe_offset=0] |= CONTEXT_CI_EXIT_CODE
        output[unsafe_offset=5] = exit_code[0]
        output[unsafe_offset=6] = exit_code[1]


def context_text_line_counts(
    counts: Pointer[mut=False, Int64, _], line: Int64
) -> InlineArray[Int64, 7]:
    var offset = line * CONTEXT_SIGNAL_COUNTER_COUNT
    var values = InlineArray[Int64, 7](fill=0)
    values[0] = counts[unsafe_offset=offset]
    values[1] = counts[unsafe_offset=offset + 1]
    values[2] = counts[unsafe_offset=offset + 2]
    values[3] = counts[unsafe_offset=offset + 3]
    values[4] = counts[unsafe_offset=offset + 4]
    values[5] = counts[unsafe_offset=offset + 5]
    values[6] = counts[unsafe_offset=offset + 6]
    return values^


def context_text_counts_have_signal(values: InlineArray[Int64, 7]) -> Bool:
    return (
        values[0] > 0
        or values[1] > 0
        or values[2] > 0
        or values[3] > 0
        or values[4] > 0
        or values[5] > 0
        or values[6] > 0
    )


comptime CONTEXT_GEMINI_GLOB_MAX_BYTES: Int64 = 131_072


def context_gemini_glob_component_bounds(
    view: ProdexStringView, cursor: Int64
) -> InlineArray[Int64, 3]:
    var bounds = InlineArray[Int64, 3](fill=-1)
    var length = Int64(view.len)
    if cursor < 0 or cursor > length:
        return bounds^
    var start = cursor
    ref ptr = view.ptr.unsafe_value()
    var end = start
    while end < length and ptr[unsafe_offset=end] != 47:
        end += 1
    bounds[0] = start
    bounds[1] = end
    bounds[2] = end + 1 if end < length else length + 1
    return bounds^


def context_gemini_glob_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def context_gemini_glob_segment_matches(
    pattern: ProdexStringView,
    pattern_bounds: InlineArray[Int64, 3],
    text: ProdexStringView,
    text_bounds: InlineArray[Int64, 3],
) -> Bool:
    ref pattern_ptr = pattern.ptr.unsafe_value()
    ref text_ptr = text.ptr.unsafe_value()
    var pattern_index = pattern_bounds[0]
    var text_index = text_bounds[0]
    var pattern_end = pattern_bounds[1]
    var text_end = text_bounds[1]
    var star: Int64 = -1
    var star_text = text_index
    while text_index < text_end:
        if pattern_index < pattern_end:
            var value = pattern_ptr[unsafe_offset=pattern_index]
            if value == 42:
                star = pattern_index
                pattern_index += 1
                star_text = text_index
                continue
            if value == 63 or (
                context_gemini_glob_ascii_lower(value)
                == context_gemini_glob_ascii_lower(
                    text_ptr[unsafe_offset=text_index]
                )
            ):
                pattern_index += 1
                text_index += 1
                continue
            if star < 0:
                return False
        elif star < 0:
            return False
        pattern_index = star + 1
        star_text += 1
        text_index = star_text
    while pattern_index < pattern_end and pattern_ptr[unsafe_offset=pattern_index] == 42:
        pattern_index += 1
    return pattern_index == pattern_end


def context_gemini_glob_component_is_double_star(
    view: ProdexStringView, bounds: InlineArray[Int64, 3]
) -> Bool:
    return (
        bounds[1] - bounds[0] == 2
        and view.ptr.unsafe_value()[unsafe_offset=bounds[0]] == 42
        and view.ptr.unsafe_value()[unsafe_offset=bounds[0] + 1] == 42
    )


def context_gemini_glob_matches(
    pattern: ProdexStringView, path: ProdexStringView
) -> Bool:
    var pattern_length = Int64(pattern.len)
    var path_length = Int64(path.len)
    var pattern_cursor: Int64 = 0
    var path_cursor: Int64 = 0
    var star_pattern_cursor: Int64 = -1
    var star_path_cursor: Int64 = -1
    while path_cursor <= path_length:
        if pattern_cursor <= pattern_length:
            var pattern_bounds = context_gemini_glob_component_bounds(
                pattern, pattern_cursor
            )
            if context_gemini_glob_component_is_double_star(
                pattern, pattern_bounds
            ):
                star_pattern_cursor = pattern_bounds[2]
                pattern_cursor = pattern_bounds[2]
                star_path_cursor = path_cursor
                continue
            var path_bounds = context_gemini_glob_component_bounds(path, path_cursor)
            if context_gemini_glob_segment_matches(pattern, pattern_bounds, path, path_bounds):
                pattern_cursor = pattern_bounds[2]
                path_cursor = path_bounds[2]
                continue

        if star_pattern_cursor < 0 or star_path_cursor > path_length:
            return False
        var star_bounds = context_gemini_glob_component_bounds(path, star_path_cursor)
        star_path_cursor = star_bounds[2]
        path_cursor = star_path_cursor
        pattern_cursor = star_pattern_cursor
    while pattern_cursor <= pattern_length:
        var bounds = context_gemini_glob_component_bounds(pattern, pattern_cursor)
        if not context_gemini_glob_component_is_double_star(pattern, bounds):
            return False
        pattern_cursor = bounds[2]
    return True


@export("prodex_mojo_text_abi_version")
def prodex_mojo_text_abi_version() abi("C") -> Int64:
    return CONTEXT_TEXT_ABI_VERSION


@export("prodex_mojo_text_abi_layout")
def prodex_mojo_text_abi_layout(
    output: Pointer[mut=True, UInt64, _], output_count: Int64
) abi("C") -> Int64:
    if output_count != 12:
        return 1
    output[unsafe_offset=0] = UInt64(size_of[ProdexStringView]())
    output[unsafe_offset=1] = UInt64(align_of[ProdexStringView]())
    output[unsafe_offset=2] = UInt64(
        reflect[ProdexStringView].field_offset[name="ptr"]()
    )
    output[unsafe_offset=3] = UInt64(
        reflect[ProdexStringView].field_offset[name="len"]()
    )
    output[unsafe_offset=4] = UInt64(size_of[ProdexBytesView]())
    output[unsafe_offset=5] = UInt64(align_of[ProdexBytesView]())
    output[unsafe_offset=6] = UInt64(
        reflect[ProdexBytesView].field_offset[name="ptr"]()
    )
    output[unsafe_offset=7] = UInt64(
        reflect[ProdexBytesView].field_offset[name="len"]()
    )
    output[unsafe_offset=8] = UInt64(size_of[ContextTextRowsResult]())
    output[unsafe_offset=9] = UInt64(align_of[ContextTextRowsResult]())
    output[unsafe_offset=10] = UInt64(
        reflect[ContextTextRowsResult].field_offset[name="abi_version"]()
    )
    output[unsafe_offset=11] = UInt64(
        reflect[ContextTextRowsResult].field_offset[
            name="required_hash_capacity"
        ]()
    )
    return 0


@export("prodex_context_classify_command_metadata_v1")
def prodex_context_classify_command_metadata_v1(
    abi_version: Int64,
    metadata: Pointer[mut=False, ProdexStringView, _],
    output_kind: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    output_kind[] = CONTEXT_METADATA_NO_KIND
    var view = metadata[].copy()
    if view.len > UInt(CONTEXT_METADATA_MAX_BYTES):
        return 1
    if not context_text_view_is_valid(view):
        return 2
    if view.len == 0:
        return 0
    output_kind[] = context_metadata_kind(view)
    return 0


@export("prodex_context_gemini_glob_matches_v1")
def prodex_context_gemini_glob_matches_v1(
    abi_version: Int64,
    pattern: Pointer[mut=False, ProdexStringView, _],
    path: Pointer[mut=False, ProdexStringView, _],
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    output[] = 0
    var pattern_view = pattern[].copy()
    var path_view = path[].copy()
    if (
        pattern_view.len > UInt(CONTEXT_GEMINI_GLOB_MAX_BYTES)
        or path_view.len > UInt(CONTEXT_GEMINI_GLOB_MAX_BYTES)
    ):
        return 1
    if not context_text_view_is_valid(pattern_view) or not context_text_view_is_valid(
        path_view
    ):
        return 2
    if context_gemini_glob_matches(pattern_view, path_view):
        output[] = 1
    return 0


@export("prodex_context_classify_ci_line_v1")
def prodex_context_classify_ci_line_v1(
    abi_version: Int64,
    line: Pointer[mut=False, ProdexStringView, _],
    output: Pointer[mut=True, Int64, _],
    output_count: Int64,
) abi("C") -> Int64:
    if output_count != CONTEXT_CI_RESULT_WIDTH:
        return 1
    for index in range(CONTEXT_CI_RESULT_WIDTH):
        output[unsafe_offset=index] = -1
    output[unsafe_offset=0] = 0
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    var view = line[].copy()
    if not context_text_view_is_valid(view):
        return 2
    if view.len == 0:
        return 0
    context_ci_line_semantics(view.ptr.unsafe_value(), Int64(view.len), output)
    return 0


@export("prodex_context_prepare_signal_rows_v1")
def prodex_context_prepare_signal_rows_v1(
    abi_version: Int64,
    before_views: Pointer[mut=False, ProdexStringView, _],
    before_counts: Pointer[mut=False, Int64, _],
    before_count: Int64,
    after_views: Pointer[mut=False, ProdexStringView, _],
    after_counts: Pointer[mut=False, Int64, _],
    after_count: Int64,
    before_rows: Pointer[mut=True, Int64, _],
    before_rows_capacity: Int64,
    after_available: Pointer[mut=True, Int64, _],
    key_capacity: Int64,
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
    key_hashes: Pointer[mut=True, UInt64, _],
    key_sources: Pointer[mut=True, Int64, _],
    key_indices: Pointer[mut=True, Int64, _],
    result: Pointer[mut=True, ContextTextRowsResult, _],
) abi("C") -> Int64:
    if abi_version != CONTEXT_TEXT_ABI_VERSION:
        return 4
    if (
        before_count < 0
        or before_count > CONTEXT_SIGNAL_MAX_LINES
        or after_count < 0
        or after_count > CONTEXT_SIGNAL_MAX_LINES
        or key_capacity < 0
        or key_capacity > CONTEXT_SIGNAL_MAX_KEYS
        or hash_capacity < 1
    ):
        return 1

    var required_before_rows = before_count * CONTEXT_SIGNAL_ROW_WIDTH
    var total_lines = before_count + after_count
    var required_key_capacity = total_lines
    if required_key_capacity > CONTEXT_SIGNAL_MAX_KEYS:
        required_key_capacity = CONTEXT_SIGNAL_MAX_KEYS
    var required_hash_capacity = context_text_required_hash_capacity(
        required_key_capacity
    )
    result[] = ContextTextRowsResult(
        CONTEXT_TEXT_ABI_VERSION,
        before_count,
        after_count,
        0,
        0,
        0,
        required_before_rows,
        required_key_capacity,
        required_hash_capacity,
    )
    if (
        before_rows_capacity < required_before_rows
        or key_capacity < required_key_capacity
        or hash_capacity < required_hash_capacity
    ):
        return 1

    for line in range(before_count):
        if not context_text_view_is_valid(before_views[unsafe_offset=line]):
            return 2
        for counter in range(CONTEXT_SIGNAL_COUNTER_COUNT):
            if (
                before_counts[
                    unsafe_offset=line * CONTEXT_SIGNAL_COUNTER_COUNT + counter
                ]
                < 0
            ):
                return 2
    for line in range(after_count):
        if not context_text_view_is_valid(after_views[unsafe_offset=line]):
            return 2
        for counter in range(CONTEXT_SIGNAL_COUNTER_COUNT):
            if (
                after_counts[
                    unsafe_offset=line * CONTEXT_SIGNAL_COUNTER_COUNT + counter
                ]
                < 0
            ):
                return 2

    for slot in range(hash_capacity):
        hash_slots[unsafe_offset=slot] = -1
    var key_count: Int64 = 0
    var after_signal_line_count: Int64 = 0

    for line in range(after_count):
        var line_counts = context_text_line_counts(after_counts, line)
        if not context_text_counts_have_signal(line_counts):
            continue
        var key_id = context_text_intern(
            after_views[unsafe_offset=line],
            CONTEXT_TEXT_SOURCE_AFTER,
            line,
            before_views,
            after_views,
            hash_slots,
            key_hashes,
            key_sources,
            key_indices,
            after_available,
            Pointer(to=key_count),
            key_capacity,
            hash_capacity,
        )
        if key_id < 0:
            return 3
        if after_available[unsafe_offset=key_id] == 9223372036854775807:
            return 2
        after_available[unsafe_offset=key_id] += 1
        after_signal_line_count += 1

    for line in range(before_count):
        var line_counts = context_text_line_counts(before_counts, line)
        var key_id: Int64 = -1
        if context_text_counts_have_signal(line_counts):
            key_id = context_text_intern(
                before_views[unsafe_offset=line],
                CONTEXT_TEXT_SOURCE_BEFORE,
                line,
                before_views,
                after_views,
                hash_slots,
                key_hashes,
                key_sources,
                key_indices,
                after_available,
                Pointer(to=key_count),
                key_capacity,
                hash_capacity,
            )
            if key_id < 0:
                return 3
        before_rows[unsafe_offset=line * CONTEXT_SIGNAL_ROW_WIDTH] = key_id
        var row = line * CONTEXT_SIGNAL_ROW_WIDTH
        before_rows[unsafe_offset=row + 1] = line_counts[0]
        before_rows[unsafe_offset=row + 2] = line_counts[1]
        before_rows[unsafe_offset=row + 3] = line_counts[2]
        before_rows[unsafe_offset=row + 4] = line_counts[3]
        before_rows[unsafe_offset=row + 5] = line_counts[4]
        before_rows[unsafe_offset=row + 6] = line_counts[5]
        before_rows[unsafe_offset=row + 7] = line_counts[6]

    result[] = ContextTextRowsResult(
        CONTEXT_TEXT_ABI_VERSION,
        before_count,
        after_count,
        required_before_rows,
        key_count,
        after_signal_line_count,
        required_before_rows,
        required_key_capacity,
        required_hash_capacity,
    )
    return 0
