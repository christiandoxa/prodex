from std.memory import Pointer

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
