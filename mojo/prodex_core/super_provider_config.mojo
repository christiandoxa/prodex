from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime SUPER_PROVIDER_CONFIG_ABI_VERSION: Int64 = 1
comptime SUPER_PROVIDER_CONFIG_OK: Int64 = 0
comptime SUPER_PROVIDER_CONFIG_INVALID: Int64 = 1
comptime SUPER_PROVIDER_CONFIG_CAPACITY: Int64 = 3
comptime SUPER_PROVIDER_CONFIG_ABI: Int64 = 4
comptime SUPER_PROVIDER_CONFIG_RECORDS: Int64 = 14


def config_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def config_valid_view(address: UInt, length: Int64) -> Bool:
    if length < 0 or (length > 0 and address == 0):
        return False
    return rich_view_valid(config_view(address, length), length)


def config_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def config_range_equals[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if end - start != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(end - start):
        if config_ascii_lower(source[unsafe_offset=start + index]) != wanted[unsafe_offset=index]:
            return False
    return True


@export("prodex_runtime_bool_token_v1")
def prodex_runtime_bool_token_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return -2
    if not config_valid_view(address, length):
        return -2
    var view = config_view(address, length)
    if (
        config_range_equals["1"](view, 0, length)
        or config_range_equals["true"](view, 0, length)
        or config_range_equals["yes"](view, 0, length)
        or config_range_equals["on"](view, 0, length)
    ):
        return 1
    if (
        config_range_equals["0"](view, 0, length)
        or config_range_equals["false"](view, 0, length)
        or config_range_equals["no"](view, 0, length)
        or config_range_equals["off"](view, 0, length)
    ):
        return 0
    return -1


@export("prodex_runtime_ci_truth_token_v1")
def prodex_runtime_ci_truth_token_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return -2
    if not config_valid_view(address, length):
        return -2
    var view = config_view(address, length)
    if (
        config_range_equals["1"](view, 0, length)
        or config_range_equals["true"](view, 0, length)
        or config_range_equals["yes"](view, 0, length)
    ):
        return 1
    return 0


@export("prodex_runtime_deepseek_web_search_token_v1")
def prodex_runtime_deepseek_web_search_token_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return -2
    if not config_valid_view(address, length):
        return -2
    var view = config_view(address, length)
    if config_range_equals["auto"](view, 0, length):
        return 0
    if (
        config_range_equals["off"](view, 0, length)
        or config_range_equals["disabled"](view, 0, length)
        or config_range_equals["disable"](view, 0, length)
    ):
        return 1
    if (
        config_range_equals["openai_chat"](view, 0, length)
        or config_range_equals["openai-chat"](view, 0, length)
        or config_range_equals["chat"](view, 0, length)
    ):
        return 2
    if config_range_equals["anthropic"](view, 0, length):
        return 3
    return -1


@export("prodex_profile_import_source_class_v1")
def prodex_profile_import_source_class_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return -2
    if not config_valid_view(address, length):
        return -2
    var view = config_view(address, length)
    if config_range_equals["claude"](view, 0, length):
        return 0
    if config_range_equals["copilot"](view, 0, length):
        return 1
    if config_range_equals["kiro"](view, 0, length):
        return 2
    return -1


@export("prodex_runtime_openai_scalar_policy_v1")
def prodex_runtime_openai_scalar_policy_v1(
    abi_version: Int64,
    operation: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return -2
    if operation < 0 or operation > 2 or not config_valid_view(address, length):
        return -2
    var view = config_view(address, length)
    var bounds = rich_trim_bounds(view)
    if operation == 0:
        return Int64(config_range_equals["openai"](view, bounds[0], bounds[1]))
    if operation == 1:
        if bounds[1] - bounds[0] >= 5 and config_range_equals["gpt-5"](
            view, bounds[0], bounds[0] + 5
        ):
            return 1
        return Int64(
            config_range_equals["gpt-6-sol"](view, bounds[0], bounds[1])
            or config_range_equals["gpt-6-luna"](view, bounds[0], bounds[1])
            or config_range_equals["codex-auto-review"](view, bounds[0], bounds[1])
        )
    return Int64(
        config_range_equals["gpt-5.6-sol"](view, bounds[0], bounds[1])
        or config_range_equals["gpt-5.6-terra"](view, bounds[0], bounds[1])
        or config_range_equals["gpt-5.6-luna"](view, bounds[0], bounds[1])
        or config_range_equals["gpt-6-sol"](view, bounds[0], bounds[1])
        or config_range_equals["gpt-6-luna"](view, bounds[0], bounds[1])
    )


@export("prodex_runtime_model_provider_class_v1")
def prodex_runtime_model_provider_class_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return -2
    if not config_valid_view(address, length):
        return -2
    var view = config_view(address, length)
    if config_range_equals["prodex-local"](view, 0, length):
        return 0
    if config_range_equals["prodex-deepseek"](view, 0, length):
        return 1
    if config_range_equals["prodex-gemini"](view, 0, length):
        return 2
    if config_range_equals["prodex-anthropic"](view, 0, length):
        return 3
    if config_range_equals["prodex-copilot"](view, 0, length):
        return 4
    if config_range_equals["prodex-kiro"](view, 0, length):
        return 5
    return -1


@export("prodex_runtime_external_provider_class_v1")
def prodex_runtime_external_provider_class_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return -2
    if not config_valid_view(address, length):
        return -2
    var view = config_view(address, length)
    if config_range_equals["anthropic"](view, 0, length) or config_range_equals["claude"](view, 0, length):
        return 0
    if (
        config_range_equals["copilot"](view, 0, length)
        or config_range_equals["github-copilot"](view, 0, length)
        or config_range_equals["github_copilot"](view, 0, length)
    ):
        return 1
    if config_range_equals["deepseek"](view, 0, length):
        return 2
    if config_range_equals["gemini"](view, 0, length):
        return 3
    if config_range_equals["gemini-oauth"](view, 0, length):
        return 4
    if config_range_equals["kiro"](view, 0, length):
        return 5
    if config_range_equals["gemini-native"](view, 0, length):
        return 6
    if config_range_equals["antigravity"](view, 0, length):
        return 7
    return -1


@export("prodex_super_external_provider_alias_v1")
def prodex_super_external_provider_alias_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return -2
    if not config_valid_view(address, length):
        return -2
    var view = config_view(address, length)
    var bounds = rich_trim_bounds(view)
    var start = bounds[0]
    var end = bounds[1]
    if config_range_equals["anthropic"](view, start, end) or config_range_equals["claude"](view, start, end):
        return 0
    if (
        config_range_equals["copilot"](view, start, end)
        or config_range_equals["github-copilot"](view, start, end)
        or config_range_equals["github_copilot"](view, start, end)
    ):
        return 1
    if config_range_equals["deepseek"](view, start, end):
        return 2
    if config_range_equals["gemini"](view, start, end):
        return 3
    if config_range_equals["kiro"](view, start, end):
        return 4
    return -1


def config_put_byte(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: UInt8,
) -> Bool:
    if written[] < 0 or written[] >= capacity:
        return False
    output[unsafe_offset=written[]] = value
    written[] += 1
    return True


def config_put_literal[literal: StaticString](
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var n = Int64(literal.byte_length())
    if written[] < 0 or n > capacity - written[]:
        return False
    var source = literal.unsafe_ptr()
    for index in range(n):
        output[unsafe_offset=written[] + index] = source[unsafe_offset=index]
    written[] += n
    return True


def config_put_view(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var n = Int64(view.len)
    if written[] < 0 or n > capacity - written[]:
        return False
    var source = rich_view_ptr(view)
    for index in range(n):
        output[unsafe_offset=written[] + index] = source[unsafe_offset=index]
    written[] += n
    return True


def config_put_toml_string(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if not config_put_byte(output, capacity, written, 34):
        return False
    var source = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        var value = source[unsafe_offset=index]
        if value == 92 or value == 34:
            if not config_put_byte(output, capacity, written, 92):
                return False
        if not config_put_byte(output, capacity, written, value):
            return False
    return config_put_byte(output, capacity, written, 34)


def config_put_uint(
    value: UInt64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var divisor: UInt64 = 10_000_000_000_000_000_000
    var started = False
    while divisor > 0:
        var digit = (value // divisor) % 10
        if digit != 0 or started or divisor == 1:
            started = True
            if not config_put_byte(output, capacity, written, UInt8(48 + digit)):
                return False
        divisor //= 10
    return True


def config_begin_record(
    records: Pointer[mut=True, Int64, _],
    record_index: Int64,
    written: Pointer[mut=True, Int64, _],
):
    records[unsafe_offset=record_index * 2] = written[]
    records[unsafe_offset=record_index * 2 + 1] = -1


def config_end_record(
    records: Pointer[mut=True, Int64, _],
    record_index: Int64,
    written: Pointer[mut=True, Int64, _],
):
    records[unsafe_offset=record_index * 2 + 1] = (
        written[] - records[unsafe_offset=record_index * 2]
    )


def config_put_key_string[
    prefix: StaticString
](
    value: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    return (
        config_put_literal[prefix](output, capacity, written)
        and config_put_toml_string(value, output, capacity, written)
    )


def config_put_provider_key_string[
    suffix: StaticString
](
    provider_id: ProdexRichStringView,
    value: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    return (
        config_put_literal["model_providers."](output, capacity, written)
        and config_put_view(provider_id, output, capacity, written)
        and config_put_literal[suffix](output, capacity, written)
        and config_put_toml_string(value, output, capacity, written)
    )


def config_put_provider_key_literal[
    suffix: StaticString, value: StaticString
](
    provider_id: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    return (
        config_put_literal["model_providers."](output, capacity, written)
        and config_put_view(provider_id, output, capacity, written)
        and config_put_literal[suffix](output, capacity, written)
        and config_put_literal[value](output, capacity, written)
    )


@export("prodex_super_toml_string_v1")
def prodex_super_toml_string_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return SUPER_PROVIDER_CONFIG_ABI
    if (
        not config_valid_view(address, length)
        or output_capacity < 1
        or output_address == 0
        or written_address == 0
    ):
        return SUPER_PROVIDER_CONFIG_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not config_put_toml_string(
        config_view(address, length), output, output_capacity, written
    ):
        return SUPER_PROVIDER_CONFIG_CAPACITY
    return SUPER_PROVIDER_CONFIG_OK


@export("prodex_super_provider_config_v1")
def prodex_super_provider_config_v1(
    abi_version: Int64,
    provider_id_address: UInt,
    provider_id_length: Int64,
    provider_name_address: UInt,
    provider_name_length: Int64,
    base_url_address: UInt,
    base_url_length: Int64,
    model_address: UInt,
    model_length: Int64,
    web_search_address: UInt,
    web_search_length: Int64,
    context_window: UInt64,
    auto_compact_token_limit: UInt64,
    image_generation: Int64,
    output_address: UInt,
    output_capacity: Int64,
    records_address: UInt,
    record_count: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_PROVIDER_CONFIG_ABI_VERSION:
        return SUPER_PROVIDER_CONFIG_ABI
    if (
        not config_valid_view(provider_id_address, provider_id_length)
        or not config_valid_view(provider_name_address, provider_name_length)
        or not config_valid_view(base_url_address, base_url_length)
        or not config_valid_view(model_address, model_length)
        or not config_valid_view(web_search_address, web_search_length)
        or (image_generation != 0 and image_generation != 1)
        or output_capacity < 1
        or output_address == 0
        or records_address == 0
        or record_count != SUPER_PROVIDER_CONFIG_RECORDS
        or written_address == 0
    ):
        return SUPER_PROVIDER_CONFIG_INVALID

    var provider_id = config_view(provider_id_address, provider_id_length)
    var provider_name = config_view(provider_name_address, provider_name_length)
    var base_url = config_view(base_url_address, base_url_length)
    var model = config_view(model_address, model_length)
    var web_search = config_view(web_search_address, web_search_length)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var records = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(records_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0

    for index in range(SUPER_PROVIDER_CONFIG_RECORDS):
        config_begin_record(records, index, written)
        var ok = True
        if index == 0:
            ok = config_put_key_string["model_provider="](
                provider_id, output, output_capacity, written
            )
        elif index == 1:
            ok = config_put_key_string["model="](
                model, output, output_capacity, written
            )
        elif index == 2:
            ok = config_put_provider_key_string[".name="](
                provider_id, provider_name, output, output_capacity, written
            )
        elif index == 3:
            ok = config_put_provider_key_string[".base_url="](
                provider_id, base_url, output, output_capacity, written
            )
        elif index == 4:
            ok = config_put_provider_key_literal[
                ".wire_api=", "\"responses\""
            ](provider_id, output, output_capacity, written)
        elif index == 5:
            ok = config_put_provider_key_literal[
                ".requires_openai_auth=", "true"
            ](provider_id, output, output_capacity, written)
        elif index == 6:
            ok = config_put_provider_key_literal[
                ".supports_websockets=", "false"
            ](provider_id, output, output_capacity, written)
        elif index == 7:
            ok = (
                config_put_literal["model_context_window="](
                    output, output_capacity, written
                )
                and config_put_uint(
                    context_window, output, output_capacity, written
                )
            )
        elif index == 8:
            ok = (
                config_put_literal["model_auto_compact_token_limit="](
                    output, output_capacity, written
                )
                and config_put_uint(
                    auto_compact_token_limit, output, output_capacity, written
                )
            )
        elif index == 9:
            ok = config_put_literal[
                "model_reasoning_summary=\"none\""
            ](output, output_capacity, written)
        elif index == 10:
            ok = config_put_key_string["web_search="](
                web_search, output, output_capacity, written
            )
        elif index == 11:
            ok = config_put_literal["features.apps=false"](
                output, output_capacity, written
            )
        elif index == 12:
            ok = config_put_literal["features.js_repl=false"](
                output, output_capacity, written
            )
        else:
            ok = (
                config_put_literal["features.image_generation="](
                    output, output_capacity, written
                )
                and (
                    config_put_literal["true"](
                        output, output_capacity, written
                    )
                    if image_generation == 1
                    else config_put_literal["false"](
                        output, output_capacity, written
                    )
                )
            )
        if not ok:
            return SUPER_PROVIDER_CONFIG_CAPACITY
        config_end_record(records, index, written)
    return SUPER_PROVIDER_CONFIG_OK
