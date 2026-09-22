from std.memory import Pointer

from rich_text import (
    rich_codepoint, rich_codepoint_width, rich_view_matches_literal,
    rich_view_prefix, rich_view_ptr, rich_view_valid,
)
from rich_types import ProdexRichStringView

# Input text is borrowed only for this call. Native, non-UTF-8 OS arguments are
# represented by valid_utf8=0 and are returned by index without interpretation.
@fieldwise_init
struct LaunchArgView(Copyable, Movable):
    var address: UInt64
    var length: UInt64
    var valid_utf8: Int64

@fieldwise_init
struct LaunchArgPiece(Copyable, Movable):
    var kind: Int64
    var index: Int64
    var offset: Int64

comptime ARG_ORIGINAL: Int64 = 0
comptime ARG_PROFILE: Int64 = 1
comptime ARG_PROFILE_INLINE: Int64 = 2
comptime ARG_RESUME: Int64 = 3
comptime ARG_EXEC: Int64 = 4
comptime ARG_SESSION: Int64 = 5
comptime ARG_FULL_ACCESS: Int64 = 6


def launch_literal[value: StaticString]() -> LaunchArgView:
    return LaunchArgView(
        UInt64(Int(value.unsafe_ptr())), UInt64(value.byte_length()), 1
    )


def launch_view(value: LaunchArgView) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(value.address), UInt(value.length))


def launch_is[value: StaticString](arg: LaunchArgView) -> Bool:
    return arg.valid_utf8 == 1 and rich_view_matches_literal[value](launch_view(arg), False)


def launch_prefix[value: StaticString](arg: LaunchArgView) -> Bool:
    return arg.valid_utf8 == 1 and rich_view_prefix[value](launch_view(arg), False)


def launch_piece(plan: UInt64, index: Int64) -> LaunchArgPiece:
    if plan == 0:
        return LaunchArgPiece(ARG_ORIGINAL, index, 0)
    return Pointer[mut=False, LaunchArgPiece, MutUntrackedOrigin](
        unsafe_from_address=Int(plan)
    )[unsafe_offset=index].copy()


def launch_arg(arguments: UInt64, plan: UInt64, index: Int64) -> LaunchArgView:
    var piece = launch_piece(plan, index)
    if piece.kind == ARG_PROFILE:
        return launch_literal["--profile"]()
    if piece.kind == ARG_PROFILE_INLINE:
        # The planner needs the option shape, never its reconstructed value.
        return launch_literal["--profile="]()
    if piece.kind == ARG_RESUME:
        return launch_literal["resume"]()
    if piece.kind == ARG_EXEC:
        return launch_literal["exec"]()
    if piece.kind == ARG_FULL_ACCESS:
        return launch_literal["--dangerously-bypass-approvals-and-sandbox"]()
    if piece.kind == ARG_SESSION:
        return LaunchArgView(0, 0, 0)
    return Pointer[mut=False, LaunchArgView, MutUntrackedOrigin](
        unsafe_from_address=Int(arguments)
    )[unsafe_offset=piece.index].copy()


def launch_takes_value(arg: LaunchArgView) -> Bool:
    return (
        launch_is["-c"](arg) or launch_is["--config"](arg)
        or launch_is["-i"](arg) or launch_is["--image"](arg)
        or launch_is["-m"](arg) or launch_is["--model"](arg)
        or launch_is["--local-provider"](arg)
        or launch_is["-p"](arg) or launch_is["--profile"](arg)
        or launch_is["-P"](arg) or launch_is["-s"](arg)
        or launch_is["--sandbox"](arg) or launch_is["-C"](arg)
        or launch_is["--cd"](arg) or launch_is["--add-dir"](arg)
        or launch_is["-a"](arg) or launch_is["--ask-for-approval"](arg)
        or launch_is["--enable"](arg) or launch_is["--disable"](arg)
        or launch_is["--remote"](arg) or launch_is["--remote-auth-token-env"](arg)
        or launch_is["--output-schema"](arg) or launch_is["-o"](arg)
        or launch_is["--output-last-message"](arg) or launch_is["--color"](arg)
        or launch_is["--thread-source"](arg)
    )


def launch_first(arguments: UInt64, plan: UInt64, start: Int64, end: Int64) -> Int64:
    var index = start
    while index < end:
        var arg = launch_arg(arguments, plan, index)
        if arg.valid_utf8 == 0:
            return index
        if launch_is["--"](arg):
            return -1
        if launch_is["-"](arg):
            return index
        if launch_takes_value(arg):
            index += 2
        elif launch_prefix["-"](arg):
            index += 1
        else:
            return index
    return -1


def launch_separator(arguments: UInt64, plan: UInt64, start: Int64, end: Int64) -> Int64:
    for index in range(start, end):
        if launch_is["--"](launch_arg(arguments, plan, index)):
            return index
    return end


def launch_uuid(arg: LaunchArgView) -> Bool:
    if arg.valid_utf8 == 0 or arg.length != 36:
        return False
    var ptr = rich_view_ptr(launch_view(arg))
    for index in range(36):
        var value = ptr[unsafe_offset=index]
        if index == 8 or index == 13 or index == 18 or index == 23:
            if value != 45:
                return False
        elif not (value >= 48 and value <= 57 or value >= 65 and value <= 70 or value >= 97 and value <= 102):
            return False
    return True


def launch_contains_equals(arg: LaunchArgView, start: Int64) -> Bool:
    if arg.valid_utf8 == 0:
        return False
    var ptr = rich_view_ptr(launch_view(arg))
    for index in range(start, Int64(arg.length)):
        if ptr[unsafe_offset=index] == 61:
            return True
    return False


def launch_inline_config(arg: LaunchArgView) -> Bool:
    if launch_prefix["--config="](arg):
        return launch_contains_equals(arg, 9)
    if launch_prefix["-c"](arg):
        return launch_contains_equals(arg, 2)
    return False


def launch_rust_space(codepoint: Int64) -> Bool:
    # Rust str::trim uses Unicode White_Space, not Python's extra U+001C..1F.
    return (
        codepoint >= 9 and codepoint <= 13 or codepoint == 32
        or codepoint == 0x85 or codepoint == 0xA0 or codepoint == 0x1680
        or codepoint >= 0x2000 and codepoint <= 0x200A
        or codepoint == 0x2028 or codepoint == 0x2029 or codepoint == 0x202F
        or codepoint == 0x205F or codepoint == 0x3000
    )


def launch_nonblank(arg: LaunchArgView, start: Int64) -> Bool:
    var ptr = rich_view_ptr(launch_view(arg))
    var index = start
    while index < Int64(arg.length):
        var width = rich_codepoint_width(ptr[unsafe_offset=index])
        if not launch_rust_space(rich_codepoint(ptr, index, width)):
            return True
        index += width
    return False


def launch_resume(arguments: UInt64, plan: UInt64, count: Int64) -> Int64:
    var first = launch_first(arguments, plan, 0, count)
    if first < 0:
        return -1
    var arg = launch_arg(arguments, plan, first)
    if launch_is["resume"](arg):
        return first
    if launch_is["exec"](arg):
        var nested = launch_first(arguments, plan, first + 1, count)
        if nested >= 0 and launch_is["resume"](launch_arg(arguments, plan, nested)):
            return nested
    return -1


def launch_inspect(arguments: UInt64, count: Int64, metadata: Pointer[mut=True, Int64, _]):
    var first = launch_first(arguments, 0, 0, count)
    var resume = launch_resume(arguments, 0, count)
    metadata[unsafe_offset=0] = first
    metadata[unsafe_offset=1] = resume
    var session: Int64 = -1
    if resume >= 0:
        session = launch_first(arguments, 0, resume + 1, count)
        if session >= 0:
            if launch_arg(arguments, 0, session).valid_utf8 == 0:
                session = -1
            else:
                for index in range(resume + 1, session):
                    if launch_is["--last"](launch_arg(arguments, 0, index)):
                        session = -1
                        break
    metadata[unsafe_offset=2] = session
    metadata[unsafe_offset=3] = first if first >= 0 else launch_separator(arguments, 0, 0, count)
    var governed = first if first >= 0 else count
    var is_exec = False
    if first >= 0:
        var command = launch_arg(arguments, 0, first)
        is_exec = launch_is["exec"](command)
        if is_exec or launch_is["resume"](command):
            governed = launch_first(arguments, 0, first + 1, count)
            if governed < 0:
                governed = count
            elif is_exec and launch_is["resume"](launch_arg(arguments, 0, governed)):
                governed = launch_first(arguments, 0, governed + 1, count)
                if governed < 0:
                    governed = count
    metadata[unsafe_offset=4] = governed
    metadata[unsafe_offset=5] = Int64(is_exec)
    metadata[unsafe_offset=6] = 0
    metadata[unsafe_offset=7] = 0
    for index in range(launch_separator(arguments, 0, 0, count)):
        var arg = launch_arg(arguments, 0, index)
        if launch_is["review"](arg):
            metadata[unsafe_offset=6] = 1
        if launch_is["--dry-run"](arg):
            metadata[unsafe_offset=7] = 1
    metadata[unsafe_offset=8] = -1
    metadata[unsafe_offset=9] = 0
    metadata[unsafe_offset=10] = 0
    var index: Int64 = 0
    while index < count:
        var arg = launch_arg(arguments, 0, index)
        var offset: Int64 = -1
        if launch_is["--model"](arg) or launch_is["-m"](arg):
            index += 1
            if index >= count:
                break
            arg = launch_arg(arguments, 0, index)
            if arg.valid_utf8 == 1:
                offset = 0
        elif launch_prefix["--model="](arg):
            offset = 8
        elif launch_prefix["-m"](arg) and arg.length > 2:
            offset = 2
            var ptr = rich_view_ptr(launch_view(arg))
            while offset < Int64(arg.length) and ptr[unsafe_offset=offset] == 61:
                offset += 1
        if offset >= 0 and launch_nonblank(arg, offset):
            metadata[unsafe_offset=8] = index
            metadata[unsafe_offset=9] = offset
            metadata[unsafe_offset=10] = Int64(arg.length) - offset
            break
        index += 1
