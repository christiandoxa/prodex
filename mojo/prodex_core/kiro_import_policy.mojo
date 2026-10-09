comptime KIRO_IMPORT_POLICY_ABI_VERSION: Int64 = 1

comptime KIRO_IMPORT_POLICY_INVALID: Int64 = -1
comptime KIRO_IMPORT_POLICY_NONE: Int64 = -2
comptime KIRO_IMPORT_POLICY_ABI: Int64 = -4

comptime KIRO_AUTH_SOURCE_SOCIAL: Int64 = 0
comptime KIRO_AUTH_SOURCE_EXTERNAL_IDP: Int64 = 1
comptime KIRO_AUTH_SOURCE_FALLBACK: Int64 = 2

comptime KIRO_AUTH_KIND_SOCIAL: Int64 = 0
comptime KIRO_AUTH_KIND_EXTERNAL_IDP: Int64 = 1
comptime KIRO_AUTH_KIND_IDENTITY_CENTER: Int64 = 2
comptime KIRO_AUTH_KIND_BUILDER_ID: Int64 = 3

comptime KIRO_IMPORT_ACTION_CREATE: Int64 = 0
comptime KIRO_IMPORT_ACTION_UPDATE: Int64 = 1
comptime KIRO_IMPORT_ACTION_INVALID_NAME: Int64 = -2
comptime KIRO_IMPORT_ACTION_NAME_MISMATCH: Int64 = -3

comptime KIRO_AUTH_STORE_USE_INCOMING: Int64 = 0
comptime KIRO_AUTH_STORE_USE_STORED: Int64 = 1


def kiro_import_bool(value: Int64) -> Bool:
    return value == 0 or value == 1


@export("prodex_kiro_auth_key_choice_v1")
def prodex_kiro_auth_key_choice_v1(
    abi_version: Int64,
    priority_presence_mask: Int64,
) abi("C") -> Int64:
    if abi_version != KIRO_IMPORT_POLICY_ABI_VERSION:
        return KIRO_IMPORT_POLICY_ABI
    if priority_presence_mask < 0 or priority_presence_mask > 7:
        return KIRO_IMPORT_POLICY_INVALID
    for index in range(3):
        if priority_presence_mask & (Int64(1) << Int64(index)) != 0:
            return Int64(index)
    return KIRO_IMPORT_POLICY_NONE


@export("prodex_kiro_auth_kind_v1")
def prodex_kiro_auth_kind_v1(
    abi_version: Int64,
    auth_source: Int64,
    start_url_present: Int64,
    start_url_is_builder: Int64,
) abi("C") -> Int64:
    if abi_version != KIRO_IMPORT_POLICY_ABI_VERSION:
        return KIRO_IMPORT_POLICY_ABI
    if (
        auth_source < KIRO_AUTH_SOURCE_SOCIAL
        or auth_source > KIRO_AUTH_SOURCE_FALLBACK
        or not kiro_import_bool(start_url_present)
        or not kiro_import_bool(start_url_is_builder)
        or start_url_is_builder == 1 and start_url_present == 0
    ):
        return KIRO_IMPORT_POLICY_INVALID
    if auth_source == KIRO_AUTH_SOURCE_SOCIAL:
        return KIRO_AUTH_KIND_SOCIAL
    if auth_source == KIRO_AUTH_SOURCE_EXTERNAL_IDP:
        return KIRO_AUTH_KIND_EXTERNAL_IDP
    if start_url_present == 1 and start_url_is_builder == 0:
        return KIRO_AUTH_KIND_IDENTITY_CENTER
    return KIRO_AUTH_KIND_BUILDER_ID


@export("prodex_kiro_profile_match_v1")
def prodex_kiro_profile_match_v1(
    abi_version: Int64,
    records_address: UInt,
    record_count: Int64,
) abi("C") -> Int64:
    if abi_version != KIRO_IMPORT_POLICY_ABI_VERSION:
        return KIRO_IMPORT_POLICY_ABI
    if record_count < 0 or record_count > 65_536:
        return KIRO_IMPORT_POLICY_INVALID
    if record_count == 0:
        return KIRO_IMPORT_POLICY_NONE
    if records_address == 0:
        return KIRO_IMPORT_POLICY_INVALID
    var records = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(records_address)
    )
    for index in range(record_count):
        var flags = records[unsafe_offset=index]
        if flags < 0 or flags > 7:
            return KIRO_IMPORT_POLICY_INVALID
        if flags == 7:
            return Int64(index)
    return KIRO_IMPORT_POLICY_NONE


@export("prodex_kiro_import_action_v1")
def prodex_kiro_import_action_v1(
    abi_version: Int64,
    matching_profile_present: Int64,
    requested_name_present: Int64,
    requested_name_valid: Int64,
    requested_name_matches: Int64,
) abi("C") -> Int64:
    if abi_version != KIRO_IMPORT_POLICY_ABI_VERSION:
        return KIRO_IMPORT_POLICY_ABI
    if (
        not kiro_import_bool(matching_profile_present)
        or not kiro_import_bool(requested_name_present)
        or not kiro_import_bool(requested_name_valid)
        or not kiro_import_bool(requested_name_matches)
    ):
        return KIRO_IMPORT_POLICY_INVALID
    if requested_name_present == 1 and requested_name_valid == 0:
        return KIRO_IMPORT_ACTION_INVALID_NAME
    if matching_profile_present == 1:
        if requested_name_present == 1 and requested_name_matches == 0:
            return KIRO_IMPORT_ACTION_NAME_MISMATCH
        return KIRO_IMPORT_ACTION_UPDATE
    return KIRO_IMPORT_ACTION_CREATE


@export("prodex_kiro_auth_store_action_v1")
def prodex_kiro_auth_store_action_v1(
    abi_version: Int64,
    incoming_expiry_present: Int64,
    stored_auth_present: Int64,
    stored_expiry_present: Int64,
    incoming_is_newer: Int64,
) abi("C") -> Int64:
    if abi_version != KIRO_IMPORT_POLICY_ABI_VERSION:
        return KIRO_IMPORT_POLICY_ABI
    if (
        not kiro_import_bool(incoming_expiry_present)
        or not kiro_import_bool(stored_auth_present)
        or not kiro_import_bool(stored_expiry_present)
        or not kiro_import_bool(incoming_is_newer)
        or stored_expiry_present == 1 and stored_auth_present == 0
        or incoming_is_newer == 1
        and incoming_expiry_present == 0
    ):
        return KIRO_IMPORT_POLICY_INVALID
    if stored_auth_present == 1 and incoming_is_newer == 0:
        return KIRO_AUTH_STORE_USE_STORED
    return KIRO_AUTH_STORE_USE_INCOMING
