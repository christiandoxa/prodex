# Numeric schema metadata only; secret and crypto payloads remain on the host.
from std.memory import Pointer
from json_sink import JsonSink, js_literal, js_u64
from profile_export_policy import (
    PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS, PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS,
    PROFILE_EXPORT_ENVELOPE_VERSION_V1, PROFILE_EXPORT_ENVELOPE_VERSION_V2,
)


def profile_export_diagnostic(
    writer: Pointer[mut=True, JsonSink, _], issue: Int64,
    expected: Int64, received: Int64, iterations: Int64, argon2: Int64,
):
    if issue == 6:
        js_literal(writer, StringSlice("profile export PBKDF2 iteration count "))
        js_u64(writer, UInt64(max(iterations, 0)))
        js_literal(writer, StringSlice(" is outside safe range "))
        js_u64(writer, PROFILE_EXPORT_PBKDF2_MIN_ITERATIONS)
        js_literal(writer, StringSlice("..="))
        js_u64(writer, PROFILE_EXPORT_PBKDF2_MAX_ITERATIONS)
    elif issue == 7:
        js_literal(writer, StringSlice("unsupported profile export Argon2id version "))
        js_u64(writer, UInt64(max(argon2, 0)))
    elif issue == 8:
        js_literal(writer, StringSlice("profile export Argon2id memory cost is outside safe range"))
    elif issue == 9:
        js_literal(writer, StringSlice("profile export Argon2id iteration count is outside safe range"))
    elif issue == 10:
        js_literal(writer, StringSlice("profile export Argon2id parallelism is outside safe range"))
    elif issue == 11:
        js_literal(writer, StringSlice("unsupported profile export format"))
    elif issue == 12:
        js_literal(writer, StringSlice("unsupported profile export version "))
        js_u64(writer, UInt64(received))
        js_literal(writer, StringSlice(" (supported: "))
        js_u64(writer, UInt64(PROFILE_EXPORT_ENVELOPE_VERSION_V1))
        js_literal(writer, StringSlice(", "))
        js_u64(writer, UInt64(PROFILE_EXPORT_ENVELOPE_VERSION_V2))
        js_literal(writer, StringSlice(")"))
    elif issue == 13:
        js_literal(writer, StringSlice("unsupported profile export cipher"))
    elif issue == 14:
        js_literal(writer, StringSlice("unsupported profile export KDF"))
    elif issue == 15:
        js_literal(writer, StringSlice("invalid encrypted export salt encoding"))
    elif issue == 16:
        js_literal(writer, StringSlice("invalid encrypted export salt length"))
    elif issue == 17:
        js_literal(writer, StringSlice("invalid encrypted export nonce encoding"))
    elif issue == 18:
        js_literal(writer, StringSlice("invalid encrypted export nonce length"))
    elif issue == 19:
        js_literal(writer, StringSlice("invalid encrypted export payload encoding"))
    elif issue == 20 or issue == 21:
        js_literal(writer, StringSlice("invalid encrypted export payload length"))
    elif expected < 0:
        js_literal(writer, StringSlice("invalid profile export envelope version"))
    else:
        js_literal(writer, StringSlice("profile export envelope kind requires version "))
        js_u64(writer, UInt64(expected))
        js_literal(writer, StringSlice(" (received "))
        js_u64(writer, UInt64(received))
        js_literal(writer, StringSlice(")"))


@export("prodex_profile_export_diagnostic_v1")
def prodex_profile_export_diagnostic_v1(
    version: Int64, issue: Int64, expected: Int64, received: Int64,
    iterations: Int64, argon2: Int64, output_address: UInt,
    capacity: Int64, written_address: UInt,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if (issue < 6 or issue > 22 or expected < -1 or expected > 0xFFFFFFFF
        or received < 0 or received > 0xFFFFFFFF
        or iterations < -1 or iterations > 0xFFFFFFFF
        or argon2 < -1 or argon2 > 0xFFFFFFFF
        or capacity < 0 or written_address == 0 or written_address % 8 != 0):
        return 1
    var writer = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=1), 0, 0, True, False,
    )
    profile_export_diagnostic(Pointer(to=writer), issue, expected, received, iterations, argon2)
    if writer.failed or writer.written > capacity:
        return 3
    if output_address == 0:
        return 1
    writer.output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    writer.capacity = capacity
    writer.written = 0
    writer.measuring = False
    profile_export_diagnostic(Pointer(to=writer), issue, expected, received, iterations, argon2)
    if writer.failed:
        return 3
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))[] = writer.written
    return 0
