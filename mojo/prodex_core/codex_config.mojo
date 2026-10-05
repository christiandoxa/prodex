from std.memory import Pointer

from rich_text import (
    rich_trim_bounds, rich_view_matches_literal, rich_view_prefix, rich_view_valid,
)
from rich_types import ProdexRichStringView, rich_view_ptr

comptime CODEX_CONFIG_ABI_VERSION: Int64 = 1
comptime CODEX_CONFIG_OK: Int64 = 0
comptime CODEX_CONFIG_INVALID: Int64 = 1
comptime CODEX_CONFIG_ABI: Int64 = 4
comptime CODEX_PROVIDER_PLAN_READ_PROFILE: Int64 = 0
comptime CODEX_PROVIDER_PLAN_READ_CONFIG: Int64 = 1
comptime CODEX_PROVIDER_PLAN_CLI_OVERRIDE: Int64 = 2
comptime CODEX_PROVIDER_PLAN_PROFILE_CONFIG: Int64 = 3
comptime CODEX_PROVIDER_PLAN_CONFIG_FILE: Int64 = 4
comptime CODEX_PROVIDER_PLAN_NO_PROVIDER: Int64 = 5

@fieldwise_init
struct CodexConfigArgView(Copyable, Movable):
    var address: UInt64
    var length: UInt64
    var valid_utf8: Int64


def codex_config_arg(arguments: UInt, index: Int64) -> CodexConfigArgView:
    return Pointer[mut=False, CodexConfigArgView, ImmUntrackedOrigin](
        unsafe_from_address=Int(arguments)
    )[unsafe_offset=index].copy()


def codex_config_view(arg: CodexConfigArgView) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(arg.address), UInt(arg.length))


def codex_config_profile_name_valid(view: ProdexRichStringView) -> Bool:
    if view.len == 0:
        return False
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        var value = ptr[unsafe_offset=index]
        if not (
            value >= 48 and value <= 57
            or value >= 65 and value <= 90
            or value >= 97 and value <= 122
            or value == 95
            or value == 45
        ):
            return False
    return True


def codex_config_view_equals_range(
    left: ProdexRichStringView,
    left_start: Int64,
    left_end: Int64,
    right: ProdexRichStringView,
) -> Bool:
    if left_start < 0 or left_end < left_start:
        return False
    if left_end - left_start != Int64(right.len):
        return False
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(left_end - left_start):
        if left_ptr[unsafe_offset=left_start + index] != right_ptr[unsafe_offset=index]:
            return False
    return True


def codex_config_contains_equals(
    view: ProdexRichStringView, start: Int64
) -> Bool:
    var ptr = rich_view_ptr(view)
    for index in range(start, Int64(view.len)):
        if ptr[unsafe_offset=index] == 61:
            return True
    return False


def codex_config_assignment_value(
    view: ProdexRichStringView,
    assignment_start: Int64,
    expected_key: ProdexRichStringView,
    raw_start: Pointer[mut=True, Int64, _],
    raw_length: Pointer[mut=True, Int64, _],
    normalized_start: Pointer[mut=True, Int64, _],
    normalized_length: Pointer[mut=True, Int64, _],
) -> Bool:
    var ptr = rich_view_ptr(view)
    var equals: Int64 = -1
    for index in range(assignment_start, Int64(view.len)):
        if ptr[unsafe_offset=index] == 61:
            equals = index
            break
    if equals < 0:
        return False

    var key_view = ProdexRichStringView(
        UInt(Int(view.ptr) + Int(assignment_start)),
        UInt(equals - assignment_start),
    )
    var key_bounds = rich_trim_bounds(key_view)
    if not codex_config_view_equals_range(
        key_view, key_bounds[0], key_bounds[1], expected_key
    ):
        return False

    raw_start[] = equals + 1
    raw_length[] = Int64(view.len) - raw_start[]

    var raw_view = ProdexRichStringView(
        UInt(Int(view.ptr) + Int(raw_start[])),
        UInt(raw_length[]),
    )
    var bounds = rich_trim_bounds(raw_view)
    var lower = raw_start[] + bounds[0]
    var upper = raw_start[] + bounds[1]

    if upper - lower >= 2:
        var first = ptr[unsafe_offset=lower]
        var last = ptr[unsafe_offset=upper - 1]
        if (first == 34 and last == 34) or (first == 39 and last == 39):
            lower += 1
            upper -= 1
            var inner = ProdexRichStringView(
                UInt(Int(view.ptr) + Int(lower)),
                UInt(upper - lower),
            )
            var inner_bounds = rich_trim_bounds(inner)
            lower += inner_bounds[0]
            upper = lower + (inner_bounds[1] - inner_bounds[0])

    if upper <= lower:
        normalized_start[] = 0
        normalized_length[] = -1
    else:
        normalized_start[] = lower
        normalized_length[] = upper - lower
    return True



def codex_config_normalized_bounds(
    view: ProdexRichStringView,
    output_start: Pointer[mut=True, Int64, _],
    output_length: Pointer[mut=True, Int64, _],
):
    var bounds = rich_trim_bounds(view)
    var lower = bounds[0]
    var upper = bounds[1]
    var ptr = rich_view_ptr(view)
    if upper - lower >= 2:
        var first = ptr[unsafe_offset=lower]
        var last = ptr[unsafe_offset=upper - 1]
        if (first == 34 and last == 34) or (first == 39 and last == 39):
            lower += 1
            upper -= 1
            var inner = ProdexRichStringView(
                UInt(Int(view.ptr) + Int(lower)),
                UInt(upper - lower),
            )
            var inner_bounds = rich_trim_bounds(inner)
            lower += inner_bounds[0]
            upper = lower + (inner_bounds[1] - inner_bounds[0])
    output_start[] = lower
    output_length[] = -1 if upper <= lower else upper - lower


@export("prodex_codex_config_normalize_value_v1")
def prodex_codex_config_normalize_value_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != CODEX_CONFIG_ABI_VERSION
        or input_length < 0
        or result_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return CODEX_CONFIG_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, input_length):
        return CODEX_CONFIG_INVALID
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    codex_config_normalized_bounds(view, result, result + 1)
    return CODEX_CONFIG_OK


@export("prodex_codex_config_profile_name_valid_v1")
def prodex_codex_config_profile_name_valid_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
) abi("C") -> Int64:
    if (
        abi_version != CODEX_CONFIG_ABI_VERSION
        or input_length < 0
        or (input_length > 0 and input_address == 0)
    ):
        return -1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, input_length):
        return -1
    return Int64(codex_config_profile_name_valid(view))


@export("prodex_codex_config_profile_v2_v1")
def prodex_codex_config_profile_v2_v1(
    abi_version: Int64,
    arguments_address: UInt,
    count: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != CODEX_CONFIG_ABI_VERSION
        or count < 0
        or result_address == 0
        or (count > 0 and arguments_address == 0)
    ):
        return CODEX_CONFIG_INVALID

    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[0] = -1
    result[1] = 0
    result[2] = 0

    var index: Int64 = 0
    while index < count:
        var arg = codex_config_arg(arguments_address, index)
        if arg.valid_utf8 == 0:
            index += 1
            continue
        var view = codex_config_view(arg)
        if not rich_view_valid(view, Int64(arg.length)):
            return CODEX_CONFIG_INVALID
        if rich_view_matches_literal["--"](view, False):
            break

        if (
            rich_view_matches_literal["--profile"](view, False)
            or rich_view_matches_literal["--profile-v2"](view, False)
        ):
            index += 1
            if index >= count:
                break
            var value_arg = codex_config_arg(arguments_address, index)
            if value_arg.valid_utf8 == 1:
                var value_view = codex_config_view(value_arg)
                if (
                    rich_view_valid(value_view, Int64(value_arg.length))
                    and codex_config_profile_name_valid(value_view)
                ):
                    result[0] = index
                    result[1] = 0
                    result[2] = Int64(value_arg.length)
                    return CODEX_CONFIG_OK
        else:
            var offset: Int64 = -1
            if rich_view_prefix["--profile="](view, False):
                offset = 10
            elif rich_view_prefix["--profile-v2="](view, False):
                offset = 13
            if offset >= 0:
                var value_view = ProdexRichStringView(
                    UInt(Int(view.ptr) + Int(offset)),
                    UInt(Int64(view.len) - offset),
                )
                if codex_config_profile_name_valid(value_view):
                    result[0] = index
                    result[1] = offset
                    result[2] = Int64(view.len) - offset
                    return CODEX_CONFIG_OK
        index += 1
    return CODEX_CONFIG_OK


@export("prodex_codex_config_override_v1")
def prodex_codex_config_override_v1(
    abi_version: Int64,
    arguments_address: UInt,
    count: Int64,
    key_address: UInt,
    key_length: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != CODEX_CONFIG_ABI_VERSION
        or count < 0
        or key_length < 0
        or result_address == 0
        or (count > 0 and arguments_address == 0)
        or (key_length > 0 and key_address == 0)
    ):
        return CODEX_CONFIG_INVALID

    var expected_key = ProdexRichStringView(key_address, UInt(key_length))
    if not rich_view_valid(expected_key, key_length):
        return CODEX_CONFIG_INVALID

    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    for slot in range(5):
        result[unsafe_offset=slot] = -1

    var index: Int64 = 0
    while index < count:
        var arg = codex_config_arg(arguments_address, index)
        if arg.valid_utf8 == 0:
            index += 1
            continue
        var view = codex_config_view(arg)
        if not rich_view_valid(view, Int64(arg.length)):
            return CODEX_CONFIG_INVALID
        if rich_view_matches_literal["--"](view, False):
            break

        var assignment_index = index
        var assignment_start: Int64 = -1
        if (
            rich_view_matches_literal["-c"](view, False)
            or rich_view_matches_literal["--config"](view, False)
        ):
            index += 1
            if index < count:
                var value_arg = codex_config_arg(arguments_address, index)
                if value_arg.valid_utf8 == 1:
                    assignment_index = index
                    view = codex_config_view(value_arg)
                    if not rich_view_valid(view, Int64(value_arg.length)):
                        return CODEX_CONFIG_INVALID
                    assignment_start = 0
        elif rich_view_prefix["--config="](view, False):
            assignment_start = 9
        elif (
            rich_view_prefix["-c"](view, False)
            and Int64(view.len) > 2
            and codex_config_contains_equals(view, 2)
        ):
            assignment_start = 2

        if assignment_start >= 0:
            var raw_start: Int64 = 0
            var raw_length: Int64 = 0
            var normalized_start: Int64 = 0
            var normalized_length: Int64 = -1
            if codex_config_assignment_value(
                view,
                assignment_start,
                expected_key,
                Pointer(to=raw_start),
                Pointer(to=raw_length),
                Pointer(to=normalized_start),
                Pointer(to=normalized_length),
            ):
                result[0] = assignment_index
                result[1] = raw_start
                result[2] = raw_length
                result[3] = normalized_start
                result[4] = normalized_length
        index += 1
    return CODEX_CONFIG_OK


@export("prodex_codex_config_model_provider_plan_v1")
def prodex_codex_config_model_provider_plan_v1(
    abi_version: Int64,
    arguments_address: UInt,
    count: Int64,
    profile_config_path_present: Int64,
    profile_config_loaded: Int64,
    config_file_loaded: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != CODEX_CONFIG_ABI_VERSION
        or count != 3
        or arguments_address == 0
        or profile_config_path_present < 0
        or profile_config_path_present > 1
        or profile_config_loaded < 0
        or profile_config_loaded > 1
        or config_file_loaded < 0
        or config_file_loaded > 1
        or result_address == 0
    ):
        return CODEX_CONFIG_INVALID

    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[0] = CODEX_PROVIDER_PLAN_NO_PROVIDER
    result[1] = 0
    result[2] = 0

    for index in range(count):
        var argument = codex_config_arg(arguments_address, index)
        if argument.valid_utf8 != 0 and argument.valid_utf8 != 1:
            return CODEX_CONFIG_INVALID
        if argument.valid_utf8 == 1 and not rich_view_valid(
            codex_config_view(argument), Int64(argument.length)
        ):
            return CODEX_CONFIG_INVALID

    var override = codex_config_arg(arguments_address, 0)
    var profile = codex_config_arg(arguments_address, 1)
    var config = codex_config_arg(arguments_address, 2)
    if profile_config_path_present == 1 and profile_config_loaded == 0:
        result[0] = CODEX_PROVIDER_PLAN_READ_PROFILE
        return CODEX_CONFIG_OK

    if profile_config_path_present == 1 and profile.valid_utf8 == 1:
        if override.valid_utf8 == 1:
            result[0] = CODEX_PROVIDER_PLAN_CLI_OVERRIDE
            result[2] = Int64(override.length)
        else:
            result[0] = CODEX_PROVIDER_PLAN_PROFILE_CONFIG
            result[2] = Int64(profile.length)
        return CODEX_CONFIG_OK

    if config_file_loaded == 0:
        result[0] = CODEX_PROVIDER_PLAN_READ_CONFIG
        return CODEX_CONFIG_OK

    if override.valid_utf8 == 1:
        result[0] = CODEX_PROVIDER_PLAN_CLI_OVERRIDE
        result[2] = Int64(override.length)
    elif config.valid_utf8 == 1:
        result[0] = CODEX_PROVIDER_PLAN_CONFIG_FILE
        result[2] = Int64(config.length)
    return CODEX_CONFIG_OK
