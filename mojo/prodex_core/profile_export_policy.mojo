from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_matches_literal, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime PROFILE_EXPORT_POLICY_ABI_VERSION: Int64 = 1
comptime PROFILE_EXPORT_POLICY_OK: Int64 = 0
comptime PROFILE_EXPORT_POLICY_INVALID: Int64 = 99
comptime PROFILE_EXPORT_POLICY_ABI: Int64 = 100

comptime PROFILE_EXPORT_POLICY_COLLECTION: Int64 = 0
comptime PROFILE_EXPORT_POLICY_PROFILE_SECRET_FILES: Int64 = 1
comptime PROFILE_EXPORT_POLICY_NESTED_SECRET_BYTES: Int64 = 2
comptime PROFILE_EXPORT_POLICY_PASSWORD_BYTES: Int64 = 3
comptime PROFILE_EXPORT_POLICY_PBKDF2_ITERATIONS: Int64 = 4
comptime PROFILE_EXPORT_POLICY_ARGON2: Int64 = 5

comptime PROFILE_EXPORT_PROFILE_COUNT: Int64 = 1
comptime PROFILE_EXPORT_SECRET_FILE_COUNT: Int64 = 2
comptime PROFILE_EXPORT_PROFILE_SECRET_FILE_COUNT: Int64 = 3
comptime PROFILE_EXPORT_NESTED_SECRET_SIZE: Int64 = 4
comptime PROFILE_EXPORT_PASSWORD_SIZE: Int64 = 5
comptime PROFILE_EXPORT_PBKDF2_RANGE: Int64 = 6
comptime PROFILE_EXPORT_ARGON2_VERSION_ERROR: Int64 = 7
comptime PROFILE_EXPORT_ARGON2_MEMORY_ERROR: Int64 = 8
comptime PROFILE_EXPORT_ARGON2_ITERATIONS_ERROR: Int64 = 9
comptime PROFILE_EXPORT_ARGON2_PARALLELISM_ERROR: Int64 = 10

comptime PROFILE_EXPORT_MAX_PROFILES: UInt64 = 256
comptime PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE: UInt64 = 16
comptime PROFILE_EXPORT_MAX_SECRET_FILES: UInt64 = 4096
comptime PROFILE_EXPORT_NESTED_JSON_MAX_BYTES: UInt64 = 2 * 1024 * 1024
comptime PROFILE_EXPORT_PASSWORD_MAX_BYTES: UInt64 = 4 * 1024
comptime PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS: UInt64 = 50_000
comptime PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS: UInt64 = 2_000_000
comptime PROFILE_EXPORT_ARGON2_VERSION: UInt64 = 0x13
comptime PROFILE_EXPORT_ARGON2_MIN_MEMORY_KIB: UInt64 = 8 * 1024
comptime PROFILE_EXPORT_ARGON2_MAX_MEMORY_KIB: UInt64 = 128 * 1024
comptime PROFILE_EXPORT_ARGON2_MIN_ITERATIONS: UInt64 = 1
comptime PROFILE_EXPORT_ARGON2_MAX_ITERATIONS: UInt64 = 6
comptime PROFILE_EXPORT_ARGON2_MIN_PARALLELISM: UInt64 = 1
comptime PROFILE_EXPORT_ARGON2_MAX_PARALLELISM: UInt64 = 4


@export("prodex_profile_export_policy_v1")
def prodex_profile_export_policy_v1(
    abi_version: Int64,
    mode: Int64,
    input0: UInt64,
    input1: UInt64,
    input2: UInt64,
    input3: UInt64,
) abi("C") -> Int64:
    if abi_version != PROFILE_EXPORT_POLICY_ABI_VERSION:
        return PROFILE_EXPORT_POLICY_ABI
    if mode < PROFILE_EXPORT_POLICY_COLLECTION or mode > PROFILE_EXPORT_POLICY_ARGON2:
        return PROFILE_EXPORT_POLICY_INVALID

    if mode == PROFILE_EXPORT_POLICY_COLLECTION:
        if input0 > PROFILE_EXPORT_MAX_PROFILES:
            return PROFILE_EXPORT_PROFILE_COUNT
        if input1 > PROFILE_EXPORT_MAX_SECRET_FILES:
            return PROFILE_EXPORT_SECRET_FILE_COUNT
        return PROFILE_EXPORT_POLICY_OK

    if mode == PROFILE_EXPORT_POLICY_PROFILE_SECRET_FILES:
        return (
            PROFILE_EXPORT_PROFILE_SECRET_FILE_COUNT
            if input0 > PROFILE_EXPORT_MAX_SECRET_FILES_PER_PROFILE
            else PROFILE_EXPORT_POLICY_OK
        )

    if mode == PROFILE_EXPORT_POLICY_NESTED_SECRET_BYTES:
        return (
            PROFILE_EXPORT_NESTED_SECRET_SIZE
            if input0 > PROFILE_EXPORT_NESTED_JSON_MAX_BYTES
            else PROFILE_EXPORT_POLICY_OK
        )

    if mode == PROFILE_EXPORT_POLICY_PASSWORD_BYTES:
        return (
            PROFILE_EXPORT_PASSWORD_SIZE
            if input0 == 0 or input0 > PROFILE_EXPORT_PASSWORD_MAX_BYTES
            else PROFILE_EXPORT_POLICY_OK
        )

    if mode == PROFILE_EXPORT_POLICY_PBKDF2_ITERATIONS:
        return (
            PROFILE_EXPORT_PBKDF2_RANGE
            if (
                input0 < PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS
                or input0 > PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS
            )
            else PROFILE_EXPORT_POLICY_OK
        )

    if input0 != PROFILE_EXPORT_ARGON2_VERSION:
        return PROFILE_EXPORT_ARGON2_VERSION_ERROR
    if (
        input1 < PROFILE_EXPORT_ARGON2_MIN_MEMORY_KIB
        or input1 > PROFILE_EXPORT_ARGON2_MAX_MEMORY_KIB
    ):
        return PROFILE_EXPORT_ARGON2_MEMORY_ERROR
    if (
        input2 < PROFILE_EXPORT_ARGON2_MIN_ITERATIONS
        or input2 > PROFILE_EXPORT_ARGON2_MAX_ITERATIONS
    ):
        return PROFILE_EXPORT_ARGON2_ITERATIONS_ERROR
    if (
        input3 < PROFILE_EXPORT_ARGON2_MIN_PARALLELISM
        or input3 > PROFILE_EXPORT_ARGON2_MAX_PARALLELISM
    ):
        return PROFILE_EXPORT_ARGON2_PARALLELISM_ERROR
    return PROFILE_EXPORT_POLICY_OK


comptime PROFILE_IMPORT_AUTH_UPDATE_APPEND: Int64 = 0
comptime PROFILE_IMPORT_AUTH_UPDATE_REPLACE_AUTH: Int64 = 1
comptime PROFILE_IMPORT_AUTH_UPDATE_REPLACE_AUTH_EMAIL: Int64 = 2


@export("prodex_profile_import_auth_update_plan_v1")
def prodex_profile_import_auth_update_plan_v1(
    abi_version: Int64,
    existing_update_present: Int64,
    incoming_email_present: Int64,
) abi("C") -> Int64:
    if abi_version != PROFILE_EXPORT_POLICY_ABI_VERSION:
        return -4
    if (
        (existing_update_present != 0 and existing_update_present != 1)
        or (incoming_email_present != 0 and incoming_email_present != 1)
    ):
        return -1
    if existing_update_present == 0:
        return PROFILE_IMPORT_AUTH_UPDATE_APPEND
    if incoming_email_present == 1:
        return PROFILE_IMPORT_AUTH_UPDATE_REPLACE_AUTH_EMAIL
    return PROFILE_IMPORT_AUTH_UPDATE_REPLACE_AUTH


comptime PROFILE_IMPORT_PLAN_ABI_VERSION: Int64 = 1
comptime PROFILE_IMPORT_PLAN_OK: Int64 = 0
comptime PROFILE_IMPORT_PLAN_INVALID: Int64 = 1
comptime PROFILE_IMPORT_PLAN_CAPACITY: Int64 = 2
comptime PROFILE_IMPORT_PLAN_ABI: Int64 = 4
comptime PROFILE_IMPORT_PLAN_EMPTY: Int64 = 5
comptime PROFILE_IMPORT_PLAN_DUPLICATE_NAME: Int64 = 6
comptime PROFILE_IMPORT_PLAN_PROVIDER_MISMATCH: Int64 = 7
comptime PROFILE_IMPORT_PLAN_LOOKUP_IDENTITY: Int64 = 8
comptime PROFILE_IMPORT_PLAN_MAX_PROFILES: Int64 = 256

comptime PROFILE_IMPORT_LOOKUP_NOT_REQUESTED: Int64 = 0
comptime PROFILE_IMPORT_LOOKUP_PENDING: Int64 = 1
comptime PROFILE_IMPORT_LOOKUP_MISSING: Int64 = 2
comptime PROFILE_IMPORT_LOOKUP_FOUND: Int64 = 3

comptime PROFILE_IMPORT_TARGET_PENDING_NEW: Int64 = 1
comptime PROFILE_IMPORT_TARGET_SOURCE: Int64 = 2
comptime PROFILE_IMPORT_TARGET_LOOKUP: Int64 = 3

comptime PROFILE_IMPORT_ACTION_UPDATE_EXISTING: Int64 = 0
comptime PROFILE_IMPORT_ACTION_STAGE_NEW: Int64 = 1
comptime PROFILE_IMPORT_ACTION_REWRITE_STAGED_AUTH: Int64 = 2


def profile_import_view_valid(view: ProdexRichStringView, allow_empty: Bool) -> Bool:
    if view.len == 0:
        return allow_empty and view.ptr == 0
    if view.ptr == 0 or view.len > UInt(9_223_372_036_854_775_807):
        return False
    return rich_view_valid(view, Int64(view.len))


def profile_import_views_equal(
    left: ProdexRichStringView,
    right: ProdexRichStringView,
) -> Bool:
    if left.len != right.len:
        return False
    if left.len == 0:
        return True
    var left_bytes = rich_view_ptr(left)
    var right_bytes = rich_view_ptr(right)
    for index in range(Int64(left.len)):
        if left_bytes[unsafe_offset=index] != right_bytes[unsafe_offset=index]:
            return False
    return True


def profile_import_plan_write_index(
    status: Int64,
    index: Int64,
    output: Pointer[mut=True, Int64, _],
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    output[unsafe_offset=0] = index
    written[] = 1
    return status


@export("prodex_profile_import_plan_v1")
def prodex_profile_import_plan_v1(
    abi_version: Int64,
    profile_count: Int64,
    names_address: UInt,
    identity_keys_address: UInt,
    lookup_names_address: UInt,
    flags_address: UInt,
    scratch_address: UInt,
    scratch_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROFILE_IMPORT_PLAN_ABI_VERSION:
        return PROFILE_IMPORT_PLAN_ABI
    if profile_count < 0 or profile_count > PROFILE_IMPORT_PLAN_MAX_PROFILES:
        return PROFILE_IMPORT_PLAN_INVALID
    if written_address == 0:
        return PROFILE_IMPORT_PLAN_INVALID
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if profile_count == 0:
        return PROFILE_IMPORT_PLAN_EMPTY
    if (
        names_address == 0
        or identity_keys_address == 0
        or lookup_names_address == 0
        or flags_address == 0
        or scratch_address == 0
        or output_address == 0
    ):
        return PROFILE_IMPORT_PLAN_INVALID
    if scratch_capacity < profile_count * 3 or output_capacity < profile_count * 5:
        return PROFILE_IMPORT_PLAN_CAPACITY

    var names = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(names_address))
    var identity_keys = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(identity_keys_address))
    var lookup_names = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(lookup_names_address))
    var flags = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(flags_address)
    )
    var scratch = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )

    for index in range(profile_count):
        var name = names[unsafe_offset=index].copy()
        var identity_key = identity_keys[unsafe_offset=index].copy()
        var lookup_name = lookup_names[unsafe_offset=index].copy()
        var supports_runtime = flags[unsafe_offset=index * 3]
        var existing_status = flags[unsafe_offset=index * 3 + 1]
        var lookup_status = flags[unsafe_offset=index * 3 + 2]
        var has_identity = identity_key.len > 0
        if (
            not profile_import_view_valid(name, False)
            or (
                has_identity
                and not profile_import_view_valid(identity_key, False)
            )
            or (not has_identity and not profile_import_view_valid(identity_key, True))
            or (supports_runtime != 0 and supports_runtime != 1)
            or existing_status < -1
            or existing_status > 1
            or lookup_status < PROFILE_IMPORT_LOOKUP_NOT_REQUESTED
            or lookup_status > PROFILE_IMPORT_LOOKUP_FOUND
        ):
            return PROFILE_IMPORT_PLAN_INVALID
        if (
            (supports_runtime == 0 or not has_identity)
            and lookup_status != PROFILE_IMPORT_LOOKUP_NOT_REQUESTED
        ):
            return PROFILE_IMPORT_PLAN_INVALID
        if (
            supports_runtime == 1
            and has_identity
            and lookup_status == PROFILE_IMPORT_LOOKUP_NOT_REQUESTED
        ):
            return PROFILE_IMPORT_PLAN_INVALID
        if lookup_status == PROFILE_IMPORT_LOOKUP_FOUND:
            if not profile_import_view_valid(lookup_name, False):
                return PROFILE_IMPORT_PLAN_INVALID
        elif not profile_import_view_valid(lookup_name, True):
            return PROFILE_IMPORT_PLAN_INVALID

    var identity_target_kind = scratch
    var identity_target_ref = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address) + Int(profile_count * 8)
    )
    var staged_source = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address) + Int(profile_count * 16)
    )
    for index in range(profile_count):
        identity_target_kind[unsafe_offset=index] = 0
        identity_target_ref[unsafe_offset=index] = -1
        staged_source[unsafe_offset=index] = -1

    var staged_count: Int64 = 0
    for index in range(profile_count):
        var name = names[unsafe_offset=index].copy()
        for previous in range(index):
            if profile_import_views_equal(name, names[unsafe_offset=previous].copy()):
                return profile_import_plan_write_index(
                    PROFILE_IMPORT_PLAN_DUPLICATE_NAME,
                    index,
                    output,
                    written,
                )

        var supports_runtime = flags[unsafe_offset=index * 3]
        var existing_status = flags[unsafe_offset=index * 3 + 1]
        var lookup_status = flags[unsafe_offset=index * 3 + 2]
        var identity_key = identity_keys[unsafe_offset=index].copy()
        var has_identity = identity_key.len > 0
        var output_offset = index * 5
        output[unsafe_offset=output_offset] = index

        if existing_status >= 0:
            if existing_status != supports_runtime:
                return profile_import_plan_write_index(
                    PROFILE_IMPORT_PLAN_PROVIDER_MISMATCH,
                    index,
                    output,
                    written,
                )
            output[unsafe_offset=output_offset + 1] =
                PROFILE_IMPORT_ACTION_UPDATE_EXISTING
            output[unsafe_offset=output_offset + 2] = -1
            output[unsafe_offset=output_offset + 3] = 0
            output[unsafe_offset=output_offset + 4] = index
            if has_identity:
                identity_target_kind[unsafe_offset=index] =
                    PROFILE_IMPORT_TARGET_SOURCE
                identity_target_ref[unsafe_offset=index] = index
            continue

        var known_identity_index: Int64 = -1
        if supports_runtime == 1 and has_identity:
            for reverse_index in range(index):
                var previous = index - reverse_index - 1
                if (
                    identity_target_kind[unsafe_offset=previous] != 0
                    and profile_import_views_equal(
                        identity_key,
                        identity_keys[unsafe_offset=previous].copy(),
                    )
                ):
                    known_identity_index = previous
                    break

        if known_identity_index >= 0:
            var known_kind = identity_target_kind[
                unsafe_offset=known_identity_index
            ]
            var known_ref = identity_target_ref[
                unsafe_offset=known_identity_index
            ]
            if known_kind == PROFILE_IMPORT_TARGET_PENDING_NEW:
                if known_ref < 0 or known_ref >= staged_count:
                    return PROFILE_IMPORT_PLAN_INVALID
                var target_source = staged_source[unsafe_offset=known_ref]
                if target_source < 0 or target_source >= index:
                    return PROFILE_IMPORT_PLAN_INVALID
                output[unsafe_offset=output_offset + 1] =
                    PROFILE_IMPORT_ACTION_REWRITE_STAGED_AUTH
                output[unsafe_offset=output_offset + 2] = known_ref
                output[unsafe_offset=output_offset + 3] = 0
                output[unsafe_offset=output_offset + 4] = target_source
            elif known_kind == PROFILE_IMPORT_TARGET_SOURCE:
                output[unsafe_offset=output_offset + 1] =
                    PROFILE_IMPORT_ACTION_UPDATE_EXISTING
                output[unsafe_offset=output_offset + 2] = -1
                output[unsafe_offset=output_offset + 3] = 0
                output[unsafe_offset=output_offset + 4] = known_ref
            elif known_kind == PROFILE_IMPORT_TARGET_LOOKUP:
                output[unsafe_offset=output_offset + 1] =
                    PROFILE_IMPORT_ACTION_UPDATE_EXISTING
                output[unsafe_offset=output_offset + 2] = -1
                output[unsafe_offset=output_offset + 3] = 1
                output[unsafe_offset=output_offset + 4] = known_ref
            else:
                return PROFILE_IMPORT_PLAN_INVALID
            continue

        if supports_runtime == 1 and has_identity:
            if lookup_status == PROFILE_IMPORT_LOOKUP_PENDING:
                return profile_import_plan_write_index(
                    PROFILE_IMPORT_PLAN_LOOKUP_IDENTITY,
                    index,
                    output,
                    written,
                )
            if lookup_status == PROFILE_IMPORT_LOOKUP_FOUND:
                output[unsafe_offset=output_offset + 1] =
                    PROFILE_IMPORT_ACTION_UPDATE_EXISTING
                output[unsafe_offset=output_offset + 2] = -1
                output[unsafe_offset=output_offset + 3] = 1
                output[unsafe_offset=output_offset + 4] = index
                identity_target_kind[unsafe_offset=index] =
                    PROFILE_IMPORT_TARGET_LOOKUP
                identity_target_ref[unsafe_offset=index] = index
                continue

        var current_staged_index = staged_count
        staged_source[unsafe_offset=current_staged_index] = index
        staged_count += 1
        output[unsafe_offset=output_offset + 1] = PROFILE_IMPORT_ACTION_STAGE_NEW
        output[unsafe_offset=output_offset + 2] = current_staged_index
        output[unsafe_offset=output_offset + 3] = 0
        output[unsafe_offset=output_offset + 4] = index
        if supports_runtime == 1 and has_identity:
            identity_target_kind[unsafe_offset=index] =
                PROFILE_IMPORT_TARGET_PENDING_NEW
            identity_target_ref[unsafe_offset=index] = current_staged_index

    written[] = profile_count * 5
    return PROFILE_IMPORT_PLAN_OK


comptime PROFILE_PASSWORD_PLAN_MODE: Int64 = 1
comptime PROFILE_PASSWORD_PLAN_SOURCE: Int64 = 2
comptime PROFILE_PASSWORD_PLAN_VALIDATE_EXPORT: Int64 = 3
comptime PROFILE_PASSWORD_PLAN_VALIDATE_IMPORT: Int64 = 4

comptime PROFILE_PASSWORD_ACTION_PROTECT: Int64 = 0
comptime PROFILE_PASSWORD_ACTION_UNPROTECTED: Int64 = 1
comptime PROFILE_PASSWORD_ACTION_PROMPT: Int64 = 2
comptime PROFILE_PASSWORD_ACTION_ENVIRONMENT: Int64 = 3
comptime PROFILE_PASSWORD_ACTION_ERROR_NON_INTERACTIVE: Int64 = 4
comptime PROFILE_PASSWORD_ACTION_VALID: Int64 = 5
comptime PROFILE_PASSWORD_ACTION_ERROR_EMPTY: Int64 = 6
comptime PROFILE_PASSWORD_ACTION_ERROR_MISMATCH: Int64 = 7


@export("prodex_profile_export_password_plan_v1")
def prodex_profile_export_password_plan_v1(
    abi_version: Int64,
    operation: Int64,
    input0: Int64,
    input1: Int64,
    input2: Int64,
) abi("C") -> Int64:
    if (
        abi_version != PROFILE_EXPORT_POLICY_ABI_VERSION
        or operation < PROFILE_PASSWORD_PLAN_MODE
        or operation > PROFILE_PASSWORD_PLAN_VALIDATE_IMPORT
        or (input0 != 0 and input0 != 1)
        or (input1 != 0 and input1 != 1)
        or (input2 != 0 and input2 != 1)
    ):
        return -1

    if operation == PROFILE_PASSWORD_PLAN_MODE:
        var password_protect = input0 == 1
        var no_password = input1 == 1
        var interactive = input2 == 1
        if password_protect:
            return PROFILE_PASSWORD_ACTION_PROTECT
        if no_password:
            return PROFILE_PASSWORD_ACTION_UNPROTECTED
        return (
            PROFILE_PASSWORD_ACTION_PROMPT
            if interactive
            else PROFILE_PASSWORD_ACTION_ERROR_NON_INTERACTIVE
        )

    if operation == PROFILE_PASSWORD_PLAN_SOURCE:
        var env_nonempty = input0 == 1
        var interactive = input1 == 1
        if env_nonempty:
            return PROFILE_PASSWORD_ACTION_ENVIRONMENT
        return (
            PROFILE_PASSWORD_ACTION_PROMPT
            if interactive
            else PROFILE_PASSWORD_ACTION_ERROR_NON_INTERACTIVE
        )

    if operation == PROFILE_PASSWORD_PLAN_VALIDATE_EXPORT:
        var empty = input0 == 1
        var matches = input1 == 1
        if empty:
            return PROFILE_PASSWORD_ACTION_ERROR_EMPTY
        if not matches:
            return PROFILE_PASSWORD_ACTION_ERROR_MISMATCH
        return PROFILE_PASSWORD_ACTION_VALID

    if input0 == 1:
        return PROFILE_PASSWORD_ACTION_ERROR_EMPTY
    return PROFILE_PASSWORD_ACTION_VALID


@export("prodex_profile_export_selection_v1")
def prodex_profile_export_selection_v1(
    abi_version: Int64,
    available_address: UInt,
    available_count: Int64,
    requested_address: UInt,
    requested_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    """Select available indices in request order; reject the first unknown name.

    Available views arrive in host BTreeSet order. Status 1 means no profiles;
    status 2 returns the missing request index in output[0]. No partial selection
    is accepted on errors. Caller owns all views and output for the call lifetime.
    """
    if abi_version != 1:
        return 100
    var required_capacity = available_count
    if requested_count > required_capacity:
        required_capacity = requested_count
    if required_capacity < 1:
        required_capacity = 1
    if (
        available_count < 0 or requested_count < 0
        or available_address == 0 or requested_address == 0
        or output_address == 0 or written_address == 0
        or output_capacity < required_capacity
    ):
        return 99
    var available = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(available_address)
    )
    var requested = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(requested_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    for index in range(available_count):
        if not profile_import_view_valid(available[unsafe_offset=index].copy(), False):
            return 99
    for index in range(requested_count):
        if not profile_import_view_valid(requested[unsafe_offset=index].copy(), False):
            return 99
    written[] = 0
    if available_count == 0:
        return 1
    if requested_count == 0:
        for index in range(available_count):
            output[unsafe_offset=index] = index
        written[] = available_count
        return 0
    for index in range(requested_count):
        var found: Int64 = -1
        for candidate in range(available_count):
            if profile_import_views_equal(
                requested[unsafe_offset=index].copy(),
                available[unsafe_offset=candidate].copy(),
            ):
                found = candidate
                break
        if found < 0:
            output[0] = index
            written[] = 1
            return 2
        var duplicate = False
        for previous in range(written[]):
            if output[unsafe_offset=previous] == found:
                duplicate = True
                break
        if not duplicate:
            output[unsafe_offset=written[]] = found
            written[] += 1
    return 0


@export("prodex_profile_export_active_profile_selected_v1")
def prodex_profile_export_active_profile_selected_v1(
    abi_version: Int64,
    active_address: UInt,
    selected_address: UInt,
    selected_count: Int64,
    is_selected_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 100
    if (
        active_address == 0
        or selected_address == 0
        or selected_count < 0
        or is_selected_address == 0
    ):
        return 99
    var active = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(active_address)
    )[].copy()
    if not profile_import_view_valid(active, True):
        return 99
    var selected = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(selected_address)
    )
    for index in range(selected_count):
        if not profile_import_view_valid(selected[unsafe_offset=index].copy(), False):
            return 99
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(is_selected_address)
    )
    output[] = 0
    if active.len == 0:
        return 0
    for index in range(selected_count):
        if profile_import_views_equal(active, selected[unsafe_offset=index].copy()):
            output[] = 1
            return 0
    return 0


comptime PROFILE_IMPORT_ACTIVE_NONE: Int64 = 0
comptime PROFILE_IMPORT_ACTIVE_EXISTING: Int64 = 1
comptime PROFILE_IMPORT_ACTIVE_RESOLVED: Int64 = 2


@export("prodex_profile_import_active_profile_plan_v1")
def prodex_profile_import_active_profile_plan_v1(
    abi_version: Int64,
    existing_address: UInt,
    source_address: UInt,
    mapping_sources_address: UInt,
    mapping_targets_address: UInt,
    mapping_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 100
    if (
        existing_address == 0
        or source_address == 0
        or mapping_sources_address == 0
        or mapping_targets_address == 0
        or mapping_count < 0
        or output_address == 0
    ):
        return 99
    var existing = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(existing_address)
    )[].copy()
    var source = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(source_address)
    )[].copy()
    if not profile_import_view_valid(existing, True) or not profile_import_view_valid(source, True):
        return 99
    var mapping_sources = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(mapping_sources_address)
    )
    var mapping_targets = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(mapping_targets_address)
    )
    for index in range(mapping_count):
        if (
            not profile_import_view_valid(mapping_sources[unsafe_offset=index].copy(), False)
            or not profile_import_view_valid(mapping_targets[unsafe_offset=index].copy(), False)
        ):
            return 99
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = PROFILE_IMPORT_ACTIVE_NONE
    output[unsafe_offset=1] = -1
    if existing.len > 0:
        output[unsafe_offset=0] = PROFILE_IMPORT_ACTIVE_EXISTING
        return 0
    if source.len == 0:
        return 0
    for index in range(mapping_count):
        if profile_import_views_equal(source, mapping_sources[unsafe_offset=index].copy()):
            output[unsafe_offset=0] = PROFILE_IMPORT_ACTIVE_RESOLVED
            output[unsafe_offset=1] = index
            return 0
    return 0


comptime PROFILE_IMPORT_LIFECYCLE_ABI_VERSION: Int64 = 1
comptime PROFILE_IMPORT_LIFECYCLE_MAX_OPERATIONS: Int64 = PROFILE_IMPORT_PLAN_MAX_PROFILES * 3


def profile_import_views_less(
    left: ProdexRichStringView,
    right: ProdexRichStringView,
) -> Bool:
    var left_bytes = rich_view_ptr(left)
    var right_bytes = rich_view_ptr(right)
    var common = min(Int64(left.len), Int64(right.len))
    for index in range(common):
        if left_bytes[unsafe_offset=index] < right_bytes[unsafe_offset=index]:
            return True
        if left_bytes[unsafe_offset=index] > right_bytes[unsafe_offset=index]:
            return False
    return left.len < right.len


@export("prodex_profile_import_lifecycle_order_v1")
def prodex_profile_import_lifecycle_order_v1(
    abi_version: Int64,
    operation_count: Int64,
    names_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    operation_count_address: UInt,
    profile_count_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROFILE_IMPORT_LIFECYCLE_ABI_VERSION:
        return 100
    if (
        operation_count < 0
        or operation_count > PROFILE_IMPORT_LIFECYCLE_MAX_OPERATIONS
        or operation_count_address == 0
        or profile_count_address == 0
    ):
        return 99
    var operation_count_out = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(operation_count_address)
    )
    var profile_count_out = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(profile_count_address)
    )
    operation_count_out[] = 0
    profile_count_out[] = 0
    if operation_count == 0:
        if output_address == 0 or output_capacity < 2:
            return 99
        var empty_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        )
        empty_output[unsafe_offset=0] = 0
        empty_output[unsafe_offset=1] = 0
        return 0
    if names_address == 0 or output_address == 0:
        return 99
    if output_capacity < operation_count * 2 + 2:
        return 2

    var names = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(names_address))
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    for index in range(operation_count):
        if not profile_import_view_valid(names[unsafe_offset=index].copy(), False):
            return 99
        output[unsafe_offset=index + 2] = index

    # ponytail: O(n²) sort is bounded by 768 import updates; use a native sort if that cap rises.
    for index in range(operation_count):
        var smallest = index
        for probe in range(index + 1, operation_count):
            var probe_index = output[unsafe_offset=probe + 2]
            var smallest_index = output[unsafe_offset=smallest + 2]
            var probe_name = names[unsafe_offset=probe_index].copy()
            var smallest_name = names[unsafe_offset=smallest_index].copy()
            if (
                profile_import_views_less(probe_name, smallest_name)
                or (
                    profile_import_views_equal(probe_name, smallest_name)
                    and probe_index < smallest_index
                )
            ):
                smallest = probe
        if smallest != index:
            var current_index = output[unsafe_offset=index + 2]
            output[unsafe_offset=index + 2] = output[unsafe_offset=smallest + 2]
            output[unsafe_offset=smallest + 2] = current_index

    var unique_count: Int64 = 0
    for index in range(operation_count):
        var current_index = output[unsafe_offset=index + 2]
        var current_name = names[unsafe_offset=current_index].copy()
        var duplicate = False
        for previous_index in range(index):
            var previous_operation = output[unsafe_offset=previous_index + 2]
            var equal = profile_import_views_equal(
                current_name,
                names[unsafe_offset=previous_operation].copy(),
            )
            if equal:
                duplicate = True
                break
        if not duplicate:
            output[unsafe_offset=operation_count + unique_count + 2] = current_index
            unique_count += 1

    output[unsafe_offset=0] = operation_count
    output[unsafe_offset=1] = unique_count
    operation_count_out[] = operation_count
    profile_count_out[] = unique_count
    return 0


@export("prodex_profile_import_secret_path_valid_v1")
def prodex_profile_import_secret_path_valid_v1(
    abi_version: Int64,
    path_address: UInt,
    path_is_absolute: Int64,
) abi("C") -> Int64:
    if abi_version != PROFILE_IMPORT_LIFECYCLE_ABI_VERSION:
        return 100
    if path_address == 0 or (path_is_absolute != 0 and path_is_absolute != 1):
        return 99
    var path = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(path_address)
    )[].copy()
    if not profile_import_view_valid(path, False):
        return 1
    var bounds = rich_trim_bounds(path)
    if bounds[0] == bounds[1] or path_is_absolute == 1:
        return 1
    var source = rich_view_ptr(path)
    for index in range(Int64(path.len)):
        var byte = source[unsafe_offset=index]
        if byte == UInt8(47) or byte == UInt8(92):
            return 1
    if path.len == UInt(1) and source[unsafe_offset=0] == UInt8(46):
        return 1
    if (
        path.len == UInt(2)
        and source[unsafe_offset=0] == UInt8(46)
        and source[unsafe_offset=1] == UInt8(46)
    ):
        return 1
    return 0


@export("prodex_profile_import_auth_journal_commit_v1")
def prodex_profile_import_auth_journal_commit_v1(
    abi_version: Int64,
    profile_exists: Int64,
    codex_home_matches: Int64,
    state_after_known: Int64,
    has_next_state: Int64,
    email_matches: Int64,
    provider_matches: Int64,
    auth_matches: Int64,
    secret_files_match: Int64,
) abi("C") -> Int64:
    if abi_version != PROFILE_IMPORT_LIFECYCLE_ABI_VERSION:
        return 100
    if (
        (profile_exists != 0 and profile_exists != 1)
        or (codex_home_matches != 0 and codex_home_matches != 1)
        or (state_after_known != 0 and state_after_known != 1)
        or (has_next_state != 0 and has_next_state != 1)
        or (email_matches != 0 and email_matches != 1)
        or (provider_matches != 0 and provider_matches != 1)
        or (auth_matches != 0 and auth_matches != 1)
        or (secret_files_match != 0 and secret_files_match != 1)
    ):
        return 99
    if (
        profile_exists == 0
        or codex_home_matches == 0
        or state_after_known == 0
        or has_next_state == 0
        or email_matches == 0
        or provider_matches == 0
        or auth_matches == 0
        or secret_files_match == 0
    ):
        return 1
    return 0


comptime PROFILE_IMPORT_RECOVERY_SKIP: Int64 = 0
comptime PROFILE_IMPORT_RECOVERY_COMMIT: Int64 = 1
comptime PROFILE_IMPORT_RECOVERY_ROLLBACK: Int64 = 2


@export("prodex_profile_import_recovery_plan_v1")
def prodex_profile_import_recovery_plan_v1(
    abi_version: Int64,
    is_removal: Int64,
    recover_removals: Int64,
    persisted_state_known: Int64,
    committed: Int64,
) abi("C") -> Int64:
    if abi_version != PROFILE_IMPORT_LIFECYCLE_ABI_VERSION:
        return PROFILE_EXPORT_POLICY_ABI
    if (
        (is_removal != 0 and is_removal != 1)
        or (recover_removals != 0 and recover_removals != 1)
        or (persisted_state_known != 0 and persisted_state_known != 1)
        or (committed != 0 and committed != 1)
    ):
        return PROFILE_EXPORT_POLICY_INVALID
    if is_removal == 1 and recover_removals == 0:
        return PROFILE_IMPORT_RECOVERY_SKIP
    if persisted_state_known == 1 and committed == 1:
        return PROFILE_IMPORT_RECOVERY_COMMIT
    return PROFILE_IMPORT_RECOVERY_ROLLBACK


comptime PROFILE_IMPORT_HOME_PROMOTE: Int64 = 0
comptime PROFILE_IMPORT_HOME_CREATE: Int64 = 1
comptime PROFILE_IMPORT_HOME_CLEANUP: Int64 = 2
comptime PROFILE_IMPORT_HOME_QUARANTINE: Int64 = 3

comptime PROFILE_IMPORT_HOME_NOOP: Int64 = 0
comptime PROFILE_IMPORT_HOME_PROMOTE_ACTION: Int64 = 1
comptime PROFILE_IMPORT_HOME_RESTORE_SOURCE: Int64 = 2
comptime PROFILE_IMPORT_HOME_CLEANUP_SOURCE: Int64 = 3
comptime PROFILE_IMPORT_HOME_CLEANUP_DESTINATION: Int64 = 4
comptime PROFILE_IMPORT_HOME_CLEANUP_BOTH: Int64 = 5


@export("prodex_profile_import_home_action_v1")
def prodex_profile_import_home_action_v1(
    abi_version: Int64,
    action_kind: Int64,
    committed: Int64,
    source_exists: Int64,
    destination_exists: Int64,
    rollback_remove: Int64,
) abi("C") -> Int64:
    if abi_version != PROFILE_IMPORT_LIFECYCLE_ABI_VERSION:
        return PROFILE_EXPORT_POLICY_ABI
    if (
        action_kind < PROFILE_IMPORT_HOME_PROMOTE
        or action_kind > PROFILE_IMPORT_HOME_QUARANTINE
        or (committed != 0 and committed != 1)
        or (source_exists != 0 and source_exists != 1)
        or (destination_exists != 0 and destination_exists != 1)
        or (rollback_remove != 0 and rollback_remove != 1)
    ):
        return PROFILE_EXPORT_POLICY_INVALID

    if action_kind == PROFILE_IMPORT_HOME_CREATE:
        return (
            PROFILE_IMPORT_HOME_NOOP
            if committed == 1
            else PROFILE_IMPORT_HOME_CLEANUP_DESTINATION
        )
    if action_kind == PROFILE_IMPORT_HOME_CLEANUP:
        return PROFILE_IMPORT_HOME_CLEANUP_DESTINATION
    if committed == 1:
        if action_kind == PROFILE_IMPORT_HOME_PROMOTE:
            if source_exists == 1 and destination_exists == 0:
                return PROFILE_IMPORT_HOME_PROMOTE_ACTION
            if source_exists == 1 and destination_exists == 1:
                return PROFILE_IMPORT_HOME_CLEANUP_SOURCE
            return PROFILE_IMPORT_HOME_NOOP
        return PROFILE_IMPORT_HOME_CLEANUP_BOTH
    if action_kind == PROFILE_IMPORT_HOME_PROMOTE and rollback_remove == 1:
        return PROFILE_IMPORT_HOME_CLEANUP_BOTH
    if source_exists == 0 and destination_exists == 1:
        return PROFILE_IMPORT_HOME_RESTORE_SOURCE
    if source_exists == 1 and destination_exists == 1:
        return PROFILE_IMPORT_HOME_CLEANUP_DESTINATION
    return PROFILE_IMPORT_HOME_NOOP


def profile_import_known_provider(view: ProdexRichStringView) -> Bool:
    return (
        rich_view_matches_literal["openai"](view, False)
        or rich_view_matches_literal["gemini"](view, False)
        or rich_view_matches_literal["anthropic"](view, False)
        or rich_view_matches_literal["copilot"](view, False)
        or rich_view_matches_literal["kiro"](view, False)
        or rich_view_matches_literal["agy"](view, False)
    )


@export("prodex_profile_import_provider_transition_v1")
def prodex_profile_import_provider_transition_v1(
    abi_version: Int64,
    source_address: UInt,
    target_address: UInt,
) abi("C") -> Int64:
    if abi_version != PROFILE_IMPORT_LIFECYCLE_ABI_VERSION:
        return PROFILE_EXPORT_POLICY_ABI
    if source_address == 0 or target_address == 0:
        return PROFILE_EXPORT_POLICY_INVALID
    var source = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(source_address)
    )[].copy()
    var target = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(target_address)
    )[].copy()
    if (
        not profile_import_view_valid(source, False)
        or not profile_import_view_valid(target, False)
        or not profile_import_known_provider(source)
        or not profile_import_known_provider(target)
    ):
        return PROFILE_EXPORT_POLICY_INVALID
    return 0 if profile_import_views_equal(source, target) else 1


@export("prodex_profile_export_copilot_strip_json_line_comments_v1")
def prodex_profile_export_copilot_strip_json_line_comments_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return PROFILE_EXPORT_POLICY_ABI
    if (
        input_length < 0
        or (input_length > 0 and input_address == 0)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return PROFILE_EXPORT_POLICY_INVALID
    if output_capacity < input_length:
        return 2
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var index: Int64 = 0
    var in_string = False
    var escaped = False
    while index < input_length:
        var value = source[unsafe_offset=index]
        if in_string:
            output[unsafe_offset=written[]] = value
            written[] += 1
            if escaped:
                escaped = False
            elif value == UInt8(92):
                escaped = True
            elif value == UInt8(34):
                in_string = False
            index += 1
            continue
        if value == UInt8(34):
            in_string = True
            output[unsafe_offset=written[]] = value
            written[] += 1
            index += 1
            continue
        if (
            value == UInt8(47)
            and index + 1 < input_length
            and source[unsafe_offset=index + 1] == UInt8(47)
        ):
            index += 2
            while index < input_length and source[unsafe_offset=index] != UInt8(10):
                index += 1
            if index < input_length:
                output[unsafe_offset=written[]] = UInt8(10)
                written[] += 1
                index += 1
            continue
        output[unsafe_offset=written[]] = value
        written[] += 1
        index += 1
    return PROFILE_EXPORT_POLICY_OK


comptime PROFILE_EXPORT_COPILOT_METADATA_VERSION: Int64 = 1
comptime PROFILE_EXPORT_COPILOT_METADATA_PLATFORM: Int64 = 2


def profile_export_copilot_version_part(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> UInt64:
    var source = rich_view_ptr(view)
    var value = UInt64(0)
    var index = start
    var saw_digit = False
    var maximum = UInt64(18446744073709551615)
    while index < end:
        var byte = source[unsafe_offset=index]
        if byte < UInt8(48) or byte > UInt8(57):
            break
        saw_digit = True
        var digit = UInt64(byte - UInt8(48))
        if value > (maximum - digit) // UInt64(10):
            return UInt64(0)
        value = value * UInt64(10) + digit
        index += 1
    return value if saw_digit else UInt64(0)


def profile_export_copilot_version(
    view: ProdexRichStringView, output: Pointer[mut=True, UInt64, _]
):
    var part = 0
    var start: Int64 = 0
    var index: Int64 = 0
    var length = Int64(view.len)
    var source = rich_view_ptr(view)
    while part < 3:
        var end = index
        while end < length and source[unsafe_offset=end] != UInt8(46):
            end += 1
        output[unsafe_offset=part] = profile_export_copilot_version_part(view, start, end)
        part += 1
        if end >= length:
            while part < 3:
                output[unsafe_offset=part] = UInt64(0)
                part += 1
            return
        index = end + 1
        start = index


def profile_export_copilot_platform(
    os: ProdexRichStringView, arch: ProdexRichStringView
) -> UInt64:
    if rich_view_matches_literal["linux"](os, False):
        if rich_view_matches_literal["aarch64"](arch, False):
            return UInt64(1)
        if rich_view_matches_literal["x86_64"](arch, False):
            return UInt64(0)
    elif rich_view_matches_literal["macos"](os, False):
        if rich_view_matches_literal["aarch64"](arch, False):
            return UInt64(3)
        if rich_view_matches_literal["x86_64"](arch, False):
            return UInt64(2)
    elif rich_view_matches_literal["windows"](os, False):
        if rich_view_matches_literal["aarch64"](arch, False):
            return UInt64(5)
        if rich_view_matches_literal["x86_64"](arch, False):
            return UInt64(4)
    return UInt64(0)


@export("prodex_profile_export_copilot_metadata_v1")
def prodex_profile_export_copilot_metadata_v1(
    abi_version: Int64,
    operation: Int64,
    primary_address: UInt,
    secondary_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return PROFILE_EXPORT_POLICY_ABI
    if primary_address == 0 or secondary_address == 0 or output_address == 0:
        return PROFILE_EXPORT_POLICY_INVALID
    var primary = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(primary_address)
    )[].copy()
    var secondary = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(secondary_address)
    )[].copy()
    if not profile_import_view_valid(primary, True) or not profile_import_view_valid(secondary, True):
        return PROFILE_EXPORT_POLICY_INVALID
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if operation == PROFILE_EXPORT_COPILOT_METADATA_VERSION:
        profile_export_copilot_version(primary, output)
        return PROFILE_EXPORT_POLICY_OK
    if operation == PROFILE_EXPORT_COPILOT_METADATA_PLATFORM:
        output[0] = profile_export_copilot_platform(primary, secondary)
        output[1] = UInt64(0)
        output[2] = UInt64(0)
        return PROFILE_EXPORT_POLICY_OK
    return PROFILE_EXPORT_POLICY_INVALID


comptime PROFILE_EXPORT_COPILOT_URL_USER_ORIGIN: Int64 = 1
comptime PROFILE_EXPORT_COPILOT_URL_MODELS: Int64 = 2
comptime PROFILE_EXPORT_COPILOT_URL_INVALID_HOST: Int64 = 3


def profile_export_copilot_write_literal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    literal: StringSlice,
) -> Bool:
    var length = Int64(literal.byte_length())
    if written[] < 0 or length > capacity - written[]:
        return False
    var source = literal.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
    return True


def profile_export_copilot_write_range(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if (
        start < 0
        or end < start
        or end > Int64(view.len)
        or written[] < 0
        or end - start > capacity - written[]
    ):
        return False
    var source = rich_view_ptr(view)
    for index in range(start, end):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
    return True


def profile_export_copilot_range_starts_with(
    view: ProdexRichStringView, start: Int64, end: Int64, literal: StringSlice
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or end > Int64(view.len) or end - start < length:
        return False
    var source = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if source[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def profile_export_copilot_range_ends_with(
    view: ProdexRichStringView, start: Int64, end: Int64, literal: StringSlice
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or end > Int64(view.len) or end - start < length:
        return False
    var source = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    var base = end - length
    for index in range(length):
        if source[unsafe_offset=base + index] != expected[unsafe_offset=index]:
            return False
    return True


def profile_export_copilot_trimmed_without_trailing_slashes(
    view: ProdexRichStringView
) -> Tuple[Int64, Int64]:
    var bounds = rich_trim_bounds(view)
    var end = bounds[1]
    var source = rich_view_ptr(view)
    while end > bounds[0] and source[unsafe_offset=end - 1] == UInt8(47):
        end -= 1
    return (bounds[0], end)


def profile_export_copilot_subview(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> ProdexRichStringView:
    return ProdexRichStringView(view.ptr + UInt(start), UInt(end - start))


def profile_export_copilot_user_origin(
    host: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    var bounds = profile_export_copilot_trimmed_without_trailing_slashes(host)
    var start = bounds[0]
    var end = bounds[1]
    if start >= end:
        return PROFILE_EXPORT_COPILOT_URL_INVALID_HOST
    var source = rich_view_ptr(host)
    var scheme_start = start
    var scheme_end = start
    var rest_start = start
    var separator: Int64 = -1
    var cursor = start
    while cursor + 2 < end:
        if (
            source[unsafe_offset=cursor] == UInt8(58)
            and source[unsafe_offset=cursor + 1] == UInt8(47)
            and source[unsafe_offset=cursor + 2] == UInt8(47)
        ):
            separator = cursor
            break
        cursor += 1
    if separator >= 0:
        scheme_end = separator
        rest_start = separator + 3
        if scheme_start == scheme_end or rest_start >= end:
            return PROFILE_EXPORT_COPILOT_URL_INVALID_HOST
    else:
        scheme_start = -1
        scheme_end = -1
        rest_start = start

    var authority_end = rest_start
    while authority_end < end:
        var value = source[unsafe_offset=authority_end]
        if value == UInt8(47) or value == UInt8(63) or value == UInt8(35):
            break
        authority_end += 1
    if authority_end == rest_start:
        return PROFILE_EXPORT_COPILOT_URL_INVALID_HOST

    var hostname_start = rest_start
    var hostname_end = authority_end
    var has_explicit_port = False
    if source[unsafe_offset=rest_start] == UInt8(91):
        var closing = rest_start + 1
        while closing < authority_end and source[unsafe_offset=closing] != UInt8(93):
            closing += 1
        if closing >= authority_end:
            return PROFILE_EXPORT_COPILOT_URL_INVALID_HOST
        hostname_start = rest_start + 1
        hostname_end = closing
        has_explicit_port = closing + 1 < authority_end and source[unsafe_offset=closing + 1] == UInt8(58)
    else:
        var colon: Int64 = -1
        var index = authority_end - 1
        while index >= rest_start:
            if source[unsafe_offset=index] == UInt8(58):
                colon = index
                break
            index -= 1
        if colon > rest_start:
            var digits = colon + 1 < authority_end
            var digit_index = colon + 1
            while digits and digit_index < authority_end:
                var byte = source[unsafe_offset=digit_index]
                if byte < UInt8(48) or byte > UInt8(57):
                    digits = False
                digit_index += 1
            if digits:
                hostname_end = colon
                has_explicit_port = True

    if hostname_start >= hostname_end:
        return PROFILE_EXPORT_COPILOT_URL_INVALID_HOST
    var hostname = profile_export_copilot_subview(host, hostname_start, hostname_end)
    var is_local = (
        rich_view_matches_literal["localhost"](hostname, False)
        or rich_view_matches_literal["127.0.0.1"](hostname, False)
        or rich_view_matches_literal["::1"](hostname, False)
    )
    var hostname_source = rich_view_ptr(hostname)
    var starts_api = (
        hostname.len >= UInt(4)
        and hostname_source[0] == UInt8(97)
        and hostname_source[1] == UInt8(112)
        and hostname_source[2] == UInt8(105)
        and hostname_source[3] == UInt8(46)
    )
    written[] = 0
    if scheme_start >= 0:
        if not profile_export_copilot_write_range(host, scheme_start, scheme_end, output, capacity, written):
            return 2
    else:
        if not profile_export_copilot_write_literal(output, capacity, written, StringSlice("https")):
            return 2
    if not profile_export_copilot_write_literal(output, capacity, written, StringSlice("://")):
        return 2
    if not has_explicit_port and not is_local and not starts_api:
        if not profile_export_copilot_write_literal(output, capacity, written, StringSlice("api.")):
            return 2
    if not profile_export_copilot_write_range(host, rest_start, authority_end, output, capacity, written):
        return 2
    return PROFILE_EXPORT_POLICY_OK


def profile_export_copilot_models_url(
    host: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    var bounds = profile_export_copilot_trimmed_without_trailing_slashes(host)
    var start = bounds[0]
    var end = bounds[1]
    var normalized = profile_export_copilot_subview(host, start, end)
    written[] = 0
    if (
        rich_view_matches_literal["https://github.com"](normalized, True)
        or rich_view_matches_literal["http://github.com"](normalized, True)
        or rich_view_matches_literal["github.com"](normalized, True)
    ):
        if not profile_export_copilot_write_literal(
            output, capacity, written, StringSlice("https://api.githubcopilot.com")
        ):
            return 2
        return PROFILE_EXPORT_POLICY_OK

    var fallback_start = start
    if profile_export_copilot_range_starts_with(host, start, end, StringSlice("https://")):
        fallback_start += 8
    elif profile_export_copilot_range_starts_with(host, start, end, StringSlice("http://")):
        fallback_start += 7
    if profile_export_copilot_range_ends_with(
        host, fallback_start, end, StringSlice(".ghe.com")
    ):
        var subdomain_end = end - 8
        if not profile_export_copilot_write_literal(
            output, capacity, written, StringSlice("https://copilot-api.")
        ):
            return 2
        if not profile_export_copilot_write_range(
            host, fallback_start, subdomain_end, output, capacity, written
        ):
            return 2
        if not profile_export_copilot_write_literal(
            output, capacity, written, StringSlice(".ghe.com")
        ):
            return 2
        return PROFILE_EXPORT_POLICY_OK
    if not profile_export_copilot_write_literal(output, capacity, written, StringSlice("https://api.")):
        return 2
    if not profile_export_copilot_write_range(host, fallback_start, end, output, capacity, written):
        return 2
    return PROFILE_EXPORT_POLICY_OK


@export("prodex_profile_export_copilot_url_v1")
def prodex_profile_export_copilot_url_v1(
    abi_version: Int64,
    operation: Int64,
    host_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return PROFILE_EXPORT_POLICY_ABI
    if host_address == 0 or output_address == 0 or output_capacity < 0 or written_address == 0:
        return PROFILE_EXPORT_POLICY_INVALID
    var host = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(host_address)
    )[].copy()
    if not profile_import_view_valid(host, True):
        return PROFILE_EXPORT_POLICY_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    if operation == PROFILE_EXPORT_COPILOT_URL_USER_ORIGIN:
        return profile_export_copilot_user_origin(host, output, output_capacity, written)
    if operation == PROFILE_EXPORT_COPILOT_URL_MODELS:
        return profile_export_copilot_models_url(host, output, output_capacity, written)
    return PROFILE_EXPORT_POLICY_INVALID


comptime PROFILE_EXPORT_COPILOT_STATE_UPDATE_EXISTING: Int64 = 0
comptime PROFILE_EXPORT_COPILOT_STATE_ADD_REQUESTED: Int64 = 1
comptime PROFILE_EXPORT_COPILOT_STATE_ADD_DEFAULT: Int64 = 2
comptime PROFILE_EXPORT_COPILOT_STATE_ACCOUNT_CONFLICT: Int64 = 3
comptime PROFILE_EXPORT_COPILOT_STATE_REQUESTED_EXISTS: Int64 = 4


@export("prodex_profile_export_copilot_import_state_v1")
def prodex_profile_export_copilot_import_state_v1(
    abi_version: Int64,
    requested_address: UInt,
    existing_address: UInt,
    has_active_profile: Int64,
    activate_requested: Int64,
    requested_name_exists: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return PROFILE_EXPORT_POLICY_ABI
    if requested_address == 0 or existing_address == 0 or output_address == 0:
        return PROFILE_EXPORT_POLICY_INVALID
    if (
        (has_active_profile != 0 and has_active_profile != 1)
        or (activate_requested != 0 and activate_requested != 1)
        or (requested_name_exists != 0 and requested_name_exists != 1)
    ):
        return PROFILE_EXPORT_POLICY_INVALID
    var requested = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(requested_address)
    )[].copy()
    var existing = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(existing_address)
    )[].copy()
    if not profile_import_view_valid(requested, True) or not profile_import_view_valid(existing, True):
        return PROFILE_EXPORT_POLICY_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=1] = 1 if has_active_profile == 0 or activate_requested != 0 else 0
    if existing.len > 0:
        if requested.len > 0 and not profile_import_views_equal(requested, existing):
            output[unsafe_offset=0] = PROFILE_EXPORT_COPILOT_STATE_ACCOUNT_CONFLICT
        else:
            output[unsafe_offset=0] = PROFILE_EXPORT_COPILOT_STATE_UPDATE_EXISTING
        return PROFILE_EXPORT_POLICY_OK
    if requested.len > 0:
        output[unsafe_offset=0] = (
            PROFILE_EXPORT_COPILOT_STATE_REQUESTED_EXISTS
            if requested_name_exists != 0
            else PROFILE_EXPORT_COPILOT_STATE_ADD_REQUESTED
        )
        return PROFILE_EXPORT_POLICY_OK
    output[unsafe_offset=0] = PROFILE_EXPORT_COPILOT_STATE_ADD_DEFAULT
    return PROFILE_EXPORT_POLICY_OK
