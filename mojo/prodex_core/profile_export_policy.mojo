from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_ptr, rich_view_valid
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
