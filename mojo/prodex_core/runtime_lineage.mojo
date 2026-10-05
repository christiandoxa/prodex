from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime LINEAGE_ABI_VERSION: Int64 = 1
comptime LINEAGE_OK: Int64 = 0
comptime LINEAGE_INVALID: Int64 = 1
comptime LINEAGE_CAPACITY: Int64 = 3
comptime COMPONENT_MAX_BYTES: Int64 = 1024
comptime KEY_MAX_BYTES: Int64 = 4096

comptime COMPACT_SESSION_PREFIX = "__compact_session__:"
comptime COMPACT_TURN_PREFIX = "__compact_turn_state__:"
comptime RESPONSE_TURN_PREFIX = "__response_turn_state__:"
comptime INVALID_SUFFIX = "__invalid__"

def view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))

def valid_text(address: UInt, length: Int64, limit: Int64) -> Bool:
    if length < 0 or length > limit or (length > 0 and address == 0):
        return False
    return rich_view_valid(view(address, length), limit)

def text_has_control(address: UInt, length: Int64) -> Bool:
    if length == 0:
        return False
    var input = view(address, length)
    var source = rich_view_ptr(input)
    var index: Int64 = 0
    while index < length:
        var width = rich_codepoint_width(source[unsafe_offset=index])
        var codepoint = rich_codepoint(source, index, width)
        if codepoint <= 31 or (codepoint >= 127 and codepoint <= 159):
            return True
        index += width
    return False

def valid_component(address: UInt, length: Int64) -> Bool:
    return (
        length > 0
        and valid_text(address, length, COMPONENT_MAX_BYTES)
        and not text_has_control(address, length)
    )

def valid_key(address: UInt, length: Int64) -> Bool:
    return (
        length > 0
        and valid_text(address, length, KEY_MAX_BYTES)
        and not text_has_control(address, length)
    )

def prefix_matches[address_literal: StaticString](
    address: UInt, length: Int64
) -> Bool:
    var n = Int64(address_literal.byte_length())
    if length < n or address == 0:
        return False
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))
    var wanted = address_literal.unsafe_ptr()
    for index in range(n):
        if source[unsafe_offset=index] != wanted[unsafe_offset=index]:
            return False
    return True

@export("prodex_runtime_lineage_classify_v1")
def prodex_runtime_lineage_classify_v1(
    abi_version: Int64,
    kind: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != LINEAGE_ABI_VERSION:
        return -1
    if kind == 0:
        return Int64(valid_component(address, length))
    if kind == 1:
        return Int64(valid_key(address, length))
    if length < 0 or (length > 0 and address == 0):
        return -1
    if kind == 2:
        return Int64(prefix_matches[RESPONSE_TURN_PREFIX](address, length))
    if kind == 3:
        return Int64(prefix_matches[COMPACT_SESSION_PREFIX](address, length))
    return -1

def emit_literal[literal: StaticString](
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var n = Int64(literal.byte_length())
    if written[] < 0 or written[] + n > capacity:
        return False
    var source = literal.unsafe_ptr()
    for index in range(n):
        output[unsafe_offset=written[] + index] = source[unsafe_offset=index]
    written[] += n
    return True

def emit_input(
    address: UInt,
    length: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if length < 0 or written[] < 0 or written[] + length > capacity:
        return False
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))
    for index in range(length):
        output[unsafe_offset=written[] + index] = source[unsafe_offset=index]
    written[] += length
    return True

def decimal_digit(value: Int64) -> UInt8:
    if value == 0:
        return 48
    if value == 1:
        return 49
    if value == 2:
        return 50
    if value == 3:
        return 51
    if value == 4:
        return 52
    if value == 5:
        return 53
    if value == 6:
        return 54
    if value == 7:
        return 55
    if value == 8:
        return 56
    return 57

def emit_decimal_component_length(
    value: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if value < 0 or value > COMPONENT_MAX_BYTES:
        return False
    var digits: Int64 = 1
    if value >= 1000:
        digits = 4
    elif value >= 100:
        digits = 3
    elif value >= 10:
        digits = 2
    if written[] + digits > capacity:
        return False
    if value >= 1000:
        output[unsafe_offset=written[]] = decimal_digit((value // 1000) % 10)
        written[] += 1
    if value >= 100:
        output[unsafe_offset=written[]] = decimal_digit((value // 100) % 10)
        written[] += 1
    if value >= 10:
        output[unsafe_offset=written[]] = decimal_digit((value // 10) % 10)
        written[] += 1
    output[unsafe_offset=written[]] = decimal_digit(value % 10)
    written[] += 1
    return True

@export("prodex_runtime_lineage_build_v1")
def prodex_runtime_lineage_build_v1(
    abi_version: Int64,
    kind: Int64,
    first_address: UInt,
    first_length: Int64,
    second_address: UInt,
    second_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != LINEAGE_ABI_VERSION
        or kind < 0
        or kind > 2
        or output_address == 0
        or written_address == 0
        or output_capacity < 1
        or output_capacity > KEY_MAX_BYTES
    ):
        return LINEAGE_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = 0

    var first_valid = valid_component(first_address, first_length)
    var second_valid = kind != 2 or valid_component(second_address, second_length)
    if kind == 0:
        if not emit_literal[COMPACT_SESSION_PREFIX](output, output_capacity, written):
            return LINEAGE_CAPACITY
    elif kind == 1:
        if not emit_literal[COMPACT_TURN_PREFIX](output, output_capacity, written):
            return LINEAGE_CAPACITY
    else:
        if not emit_literal[RESPONSE_TURN_PREFIX](output, output_capacity, written):
            return LINEAGE_CAPACITY

    if not first_valid or not second_valid:
        if not emit_literal[INVALID_SUFFIX](output, output_capacity, written):
            return LINEAGE_CAPACITY
        return LINEAGE_OK

    if kind == 0 or kind == 1:
        if not emit_input(first_address, first_length, output, output_capacity, written):
            return LINEAGE_CAPACITY
        return LINEAGE_OK

    if (
        not emit_decimal_component_length(first_length, output, output_capacity, written)
        or not emit_literal[":"](output, output_capacity, written)
        or not emit_input(first_address, first_length, output, output_capacity, written)
        or not emit_literal[":"](output, output_capacity, written)
        or not emit_input(second_address, second_length, output, output_capacity, written)
    ):
        return LINEAGE_CAPACITY
    return LINEAGE_OK

@export("prodex_runtime_lineage_parts_v1")
def prodex_runtime_lineage_parts_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LINEAGE_ABI_VERSION or output_address == 0:
        return LINEAGE_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    for index in range(4):
        output[unsafe_offset=index] = -1
    if length < 0 or (length > 0 and address == 0):
        return LINEAGE_INVALID
    if not prefix_matches[RESPONSE_TURN_PREFIX](address, length):
        return LINEAGE_OK

    var prefix_length = Int64(RESPONSE_TURN_PREFIX.byte_length())
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))
    var cursor = prefix_length
    var digits_start = cursor
    var response_length: Int64 = 0
    while cursor < length:
        var byte = source[unsafe_offset=cursor]
        if byte == 58:
            break
        if byte < 48 or byte > 57:
            return LINEAGE_OK
        var digit = Int64(byte) - 48
        if response_length <= COMPONENT_MAX_BYTES:
            response_length = response_length * 10 + digit
            if response_length > COMPONENT_MAX_BYTES:
                response_length = COMPONENT_MAX_BYTES + 1
        cursor += 1
    if cursor == digits_start or cursor >= length or source[unsafe_offset=cursor] != 58:
        return LINEAGE_OK
    if response_length > COMPONENT_MAX_BYTES:
        return LINEAGE_OK

    var response_start = cursor + 1
    var response_end = response_start + response_length
    if response_end >= length or source[unsafe_offset=response_end] != 58:
        return LINEAGE_OK
    var turn_start = response_end + 1
    var turn_end = length
    if not valid_component(
        address + UInt(response_start), response_end - response_start
    ):
        return LINEAGE_OK
    if not valid_component(address + UInt(turn_start), turn_end - turn_start):
        return LINEAGE_OK

    output[unsafe_offset=0] = response_start
    output[unsafe_offset=1] = response_end
    output[unsafe_offset=2] = turn_start
    output[unsafe_offset=3] = turn_end
    return LINEAGE_OK


comptime LINEAGE_RELEASE_RESPONSE: Int64 = 1
comptime LINEAGE_RELEASE_TURN_STATE: Int64 = 2
comptime LINEAGE_RELEASE_SESSION: Int64 = 4
comptime LINEAGE_RELEASE_COMPACT_SESSION: Int64 = 8


@export("prodex_runtime_lineage_release_plan_v1")
def prodex_runtime_lineage_release_plan_v1(
    abi_version: Int64,
    previous_response_present: Int64,
    previous_response_matches: Int64,
    turn_state_present: Int64,
    turn_state_matches: Int64,
    session_present: Int64,
    session_matches: Int64,
    compact_session_matches: Int64,
) abi("C") -> Int64:
    if abi_version != LINEAGE_ABI_VERSION:
        return -4
    for value in [
        previous_response_present,
        previous_response_matches,
        turn_state_present,
        turn_state_matches,
        session_present,
        session_matches,
        compact_session_matches,
    ]:
        if value != 0 and value != 1:
            return -1

    var mask: Int64 = 0
    if previous_response_present == 1 and previous_response_matches == 1:
        mask |= LINEAGE_RELEASE_RESPONSE
    if turn_state_present == 1 and turn_state_matches == 1:
        mask |= LINEAGE_RELEASE_TURN_STATE

    var release_session_affinity = (
        previous_response_present == 0 and turn_state_present == 0
    )
    if release_session_affinity and session_present == 1:
        if session_matches == 1:
            mask |= LINEAGE_RELEASE_SESSION
        if compact_session_matches == 1:
            mask |= LINEAGE_RELEASE_COMPACT_SESSION
    return mask


comptime LINEAGE_CANDIDATE_OWNER: Int64 = 1
comptime LINEAGE_CANDIDATE_UNAVAILABLE: Int64 = 2
comptime LINEAGE_CANDIDATE_CONFLICT: Int64 = 3

comptime LINEAGE_RESOLUTION_UNBOUND: Int64 = 0
comptime LINEAGE_RESOLUTION_OWNED: Int64 = 1
comptime LINEAGE_RESOLUTION_UNAVAILABLE: Int64 = 2
comptime LINEAGE_RESOLUTION_CONFLICT: Int64 = 3

comptime BINDING_CANDIDATE_LOCAL_REWRITE: Int64 = 0
comptime BINDING_CANDIDATE_DISPATCH: Int64 = 1


def binding_candidate_text_equal(
    left_address: UInt,
    left_length: Int64,
    right_address: UInt,
    right_length: Int64,
) -> Bool:
    if left_length < 0 or left_length != right_length:
        return False
    if left_length == 0:
        return True
    if left_address == 0 or right_address == 0:
        return False
    var left = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(left_address)
    )
    var right = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(right_address)
    )
    for index in range(left_length):
        if left[unsafe_offset=index] != right[unsafe_offset=index]:
            return False
    return True


def binding_candidate_identity_valid(
    present: Int64,
    provider: Int64,
    credential_address: UInt,
    credential_length: Int64,
    endpoint_address: UInt,
    endpoint_length: Int64,
    profile_present: Int64,
    profile_address: UInt,
    profile_length: Int64,
) -> Bool:
    if present == 0:
        return (
            provider == 0
            and credential_address == 0
            and credential_length == 0
            and endpoint_address == 0
            and endpoint_length == 0
            and profile_present == 0
            and profile_address == 0
            and profile_length == 0
        )
    if (
        provider < 0
        or provider > 6
        or credential_length <= 0
        or endpoint_length <= 0
        or not valid_text(credential_address, credential_length, 4096)
        or not valid_text(endpoint_address, endpoint_length, 4096)
        or (profile_present != 0 and profile_present != 1)
    ):
        return False
    if profile_present == 0:
        return profile_address == 0 and profile_length == 0
    return profile_length > 0 and valid_text(profile_address, profile_length, 4096)


def binding_candidate_identities_equal(
    left_provider: Int64,
    left_credential_address: UInt,
    left_credential_length: Int64,
    left_endpoint_address: UInt,
    left_endpoint_length: Int64,
    left_profile_present: Int64,
    left_profile_address: UInt,
    left_profile_length: Int64,
    right_provider: Int64,
    right_credential_address: UInt,
    right_credential_length: Int64,
    right_endpoint_address: UInt,
    right_endpoint_length: Int64,
    right_profile_present: Int64,
    right_profile_address: UInt,
    right_profile_length: Int64,
) -> Bool:
    return (
        left_provider == right_provider
        and binding_candidate_text_equal(
            left_credential_address,
            left_credential_length,
            right_credential_address,
            right_credential_length,
        )
        and binding_candidate_text_equal(
            left_endpoint_address,
            left_endpoint_length,
            right_endpoint_address,
            right_endpoint_length,
        )
        and left_profile_present == right_profile_present
        and (
            left_profile_present == 0
            or binding_candidate_text_equal(
                left_profile_address,
                left_profile_length,
                right_profile_address,
                right_profile_length,
            )
        )
    )


@export("prodex_runtime_lineage_binding_candidate_allowed_v1")
def prodex_runtime_lineage_binding_candidate_allowed_v1(
    abi_version: Int64,
    mode: Int64,
    expected_profile_address: UInt,
    expected_profile_length: Int64,
    bound_profile_present: Int64,
    bound_profile_address: UInt,
    bound_profile_length: Int64,
    bound_identity_present: Int64,
    bound_provider: Int64,
    bound_credential_address: UInt,
    bound_credential_length: Int64,
    bound_endpoint_address: UInt,
    bound_endpoint_length: Int64,
    bound_profile_identity_present: Int64,
    bound_profile_identity_address: UInt,
    bound_profile_identity_length: Int64,
    candidate_provider_present: Int64,
    candidate_provider: Int64,
    candidate_identity_present: Int64,
    candidate_identity_provider: Int64,
    candidate_credential_address: UInt,
    candidate_credential_length: Int64,
    candidate_endpoint_address: UInt,
    candidate_endpoint_length: Int64,
    candidate_profile_identity_present: Int64,
    candidate_profile_identity_address: UInt,
    candidate_profile_identity_length: Int64,
) abi("C") -> Int64:
    if abi_version != LINEAGE_ABI_VERSION:
        return -4
    if (
        (mode != BINDING_CANDIDATE_LOCAL_REWRITE and mode != BINDING_CANDIDATE_DISPATCH)
        or (bound_profile_present != 0 and bound_profile_present != 1)
        or (bound_identity_present != 0 and bound_identity_present != 1)
        or (candidate_provider_present != 0 and candidate_provider_present != 1)
        or (candidate_identity_present != 0 and candidate_identity_present != 1)
        or (bound_profile_identity_present != 0 and bound_profile_identity_present != 1)
        or (candidate_profile_identity_present != 0 and candidate_profile_identity_present != 1)
        or not binding_candidate_identity_valid(
            bound_identity_present,
            bound_provider,
            bound_credential_address,
            bound_credential_length,
            bound_endpoint_address,
            bound_endpoint_length,
            bound_profile_identity_present,
            bound_profile_identity_address,
            bound_profile_identity_length,
        )
        or not binding_candidate_identity_valid(
            candidate_identity_present,
            candidate_identity_provider,
            candidate_credential_address,
            candidate_credential_length,
            candidate_endpoint_address,
            candidate_endpoint_length,
            candidate_profile_identity_present,
            candidate_profile_identity_address,
            candidate_profile_identity_length,
        )
        or (
            candidate_provider_present == 1
            and (candidate_provider < 0 or candidate_provider > 6)
        )
        or (candidate_provider_present == 0 and candidate_provider != 0)
        or (
            mode == BINDING_CANDIDATE_LOCAL_REWRITE
            and candidate_identity_present == 1
            and candidate_provider != candidate_identity_provider
        )
    ):
        return -1

    if mode == BINDING_CANDIDATE_LOCAL_REWRITE:
        if candidate_identity_present == 0:
            return 0
        if bound_profile_present == 0:
            return 1
        if (
            expected_profile_length <= 0
            or not valid_text(expected_profile_address, expected_profile_length, 1024)
            or bound_profile_length < 0
            or not valid_text(bound_profile_address, bound_profile_length, 1024)
            or not binding_candidate_text_equal(
                expected_profile_address,
                expected_profile_length,
                bound_profile_address,
                bound_profile_length,
            )
            or bound_identity_present == 0
        ):
            return 0
        return Int64(
            binding_candidate_identities_equal(
                bound_provider,
                bound_credential_address,
                bound_credential_length,
                bound_endpoint_address,
                bound_endpoint_length,
                bound_profile_identity_present,
                bound_profile_identity_address,
                bound_profile_identity_length,
                candidate_identity_provider,
                candidate_credential_address,
                candidate_credential_length,
                candidate_endpoint_address,
                candidate_endpoint_length,
                candidate_profile_identity_present,
                candidate_profile_identity_address,
                candidate_profile_identity_length,
            )
        )

    if bound_identity_present == 0:
        return 0
    if candidate_provider_present == 0 or bound_provider != candidate_provider:
        return 1
    if candidate_identity_present == 0:
        return 0
    if binding_candidate_identities_equal(
            bound_provider,
            bound_credential_address,
            bound_credential_length,
            bound_endpoint_address,
            bound_endpoint_length,
            bound_profile_identity_present,
            bound_profile_identity_address,
            bound_profile_identity_length,
            candidate_identity_provider,
            candidate_credential_address,
            candidate_credential_length,
            candidate_endpoint_address,
            candidate_endpoint_length,
            candidate_profile_identity_present,
            candidate_profile_identity_address,
            candidate_profile_identity_length,
        ):
        return 0
    return 2


@export("prodex_runtime_lineage_candidate_plan_v1")
def prodex_runtime_lineage_candidate_plan_v1(
    abi_version: Int64,
    profile_valid: Int64,
    conflict_sentinel: Int64,
    profile_available: Int64,
    binding_identity_present: Int64,
    existing_identity_present: Int64,
    binding_identity_matches: Int64,
) abi("C") -> Int64:
    if abi_version != LINEAGE_ABI_VERSION:
        return -4
    for value in [
        profile_valid,
        conflict_sentinel,
        profile_available,
        binding_identity_present,
        existing_identity_present,
        binding_identity_matches,
    ]:
        if value != 0 and value != 1:
            return -1

    var candidate = LINEAGE_CANDIDATE_UNAVAILABLE
    if profile_valid == 0 or conflict_sentinel == 1:
        candidate = LINEAGE_CANDIDATE_CONFLICT
    elif profile_available == 1:
        candidate = LINEAGE_CANDIDATE_OWNER

    var identity_action: Int64 = 0
    if binding_identity_present == 1:
        if existing_identity_present == 0:
            identity_action = 1
        elif binding_identity_matches == 0:
            candidate = LINEAGE_CANDIDATE_CONFLICT
            identity_action = 2
    return candidate | (identity_action << 8)


@export("prodex_runtime_lineage_resolution_plan_v1")
def prodex_runtime_lineage_resolution_plan_v1(
    abi_version: Int64,
    conflict: Int64,
    owner_count: Int64,
    unavailable_count: Int64,
) abi("C") -> Int64:
    if (
        abi_version != LINEAGE_ABI_VERSION
        or (conflict != 0 and conflict != 1)
        or owner_count < 0
        or unavailable_count < 0
    ):
        return -1
    if (
        conflict == 1
        or owner_count > 1
        or unavailable_count > 1
        or (owner_count == 1 and unavailable_count > 0)
    ):
        return LINEAGE_RESOLUTION_CONFLICT
    if owner_count == 1:
        return LINEAGE_RESOLUTION_OWNED
    if unavailable_count == 1:
        return LINEAGE_RESOLUTION_UNAVAILABLE
    return LINEAGE_RESOLUTION_UNBOUND


comptime LINEAGE_LOOKUP_AFFINITY_NONE: Int64 = 0
comptime LINEAGE_LOOKUP_AFFINITY_BOUND: Int64 = 1
comptime LINEAGE_LOOKUP_AFFINITY_FALLBACK: Int64 = 2
comptime LINEAGE_LOOKUP_AFFINITY_CURRENT: Int64 = 3

comptime LINEAGE_OWNER_UNBOUND: Int64 = 0
comptime LINEAGE_OWNER_OWNED: Int64 = 1
comptime LINEAGE_OWNER_UNAVAILABLE: Int64 = 2
comptime LINEAGE_OWNER_CONFLICT: Int64 = 3

comptime LINEAGE_PROFILE_NONE: Int64 = 0
comptime LINEAGE_PROFILE_OWNER: Int64 = 1
comptime LINEAGE_PROFILE_CONFLICT_SENTINEL: Int64 = 2


@export("prodex_runtime_lineage_lookup_affinity_v1")
def prodex_runtime_lineage_lookup_affinity_v1(
    abi_version: Int64,
    turn_state_present: Int64,
    bound_present: Int64,
    fallback_present: Int64,
) abi("C") -> Int64:
    if (
        abi_version != LINEAGE_ABI_VERSION
        or (turn_state_present != 0 and turn_state_present != 1)
        or (bound_present != 0 and bound_present != 1)
        or (fallback_present != 0 and fallback_present != 1)
    ):
        return -1
    if turn_state_present == 0:
        return LINEAGE_LOOKUP_AFFINITY_NONE
    if bound_present == 1:
        return LINEAGE_LOOKUP_AFFINITY_BOUND
    if fallback_present == 1:
        return LINEAGE_LOOKUP_AFFINITY_FALLBACK
    return LINEAGE_LOOKUP_AFFINITY_CURRENT


@export("prodex_runtime_lineage_owner_lookup_v1")
def prodex_runtime_lineage_owner_lookup_v1(
    abi_version: Int64,
    owner_kind: Int64,
    expected_identity_present: Int64,
    identity_matches: Int64,
) abi("C") -> Int64:
    if (
        abi_version != LINEAGE_ABI_VERSION
        or owner_kind < LINEAGE_OWNER_UNBOUND
        or owner_kind > LINEAGE_OWNER_CONFLICT
        or (expected_identity_present != 0 and expected_identity_present != 1)
        or (identity_matches != 0 and identity_matches != 1)
    ):
        return -1

    var normalized = owner_kind
    if (
        owner_kind == LINEAGE_OWNER_OWNED
        and expected_identity_present == 1
        and identity_matches == 0
    ):
        normalized = LINEAGE_OWNER_UNAVAILABLE

    var profile_action = LINEAGE_PROFILE_NONE
    if normalized == LINEAGE_OWNER_OWNED:
        profile_action = LINEAGE_PROFILE_OWNER
    elif (
        normalized == LINEAGE_OWNER_UNAVAILABLE
        or normalized == LINEAGE_OWNER_CONFLICT
    ):
        profile_action = LINEAGE_PROFILE_CONFLICT_SENTINEL

    return normalized | (profile_action << 8)
