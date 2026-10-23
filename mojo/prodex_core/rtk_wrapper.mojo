from std.memory import Pointer
from json_sink import JsonSink, js_byte, js_literal, js_view
from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime RTKW_MAX_BYTES: Int64 = 1_048_576
comptime RTKW_MAX_ARGUMENTS: Int64 = 4_096


def rtkw_escape(sink: Pointer[mut=True, JsonSink, _], value: ProdexRichStringView, windows: Bool):
    var source = rich_view_ptr(value)
    for index in range(Int64(value.len)):
        var byte = source[unsafe_offset=index]
        if windows and byte == 37:
            js_byte(sink, 37)
        elif not windows and byte == 39:
            js_literal(sink, StringSlice("'\\''"))
            continue
        js_byte(sink, byte)


def rtkw_quote(sink: Pointer[mut=True, JsonSink, _], value: ProdexRichStringView, windows: Bool):
    js_byte(sink, UInt8(34) if windows else UInt8(39))
    rtkw_escape(sink, value, windows)
    js_byte(sink, UInt8(34) if windows else UInt8(39))


def rtkw_states(
    sink: Pointer[mut=True, JsonSink, _], windows: Bool,
    states: Pointer[mut=False, ProdexRichStringView, _], count: Int64,
):
    for index in range(count):
        var key = StringSlice("RTK_DB_PATH")
        if index == 1:
            key = StringSlice("RTK_TEE_DIR")
        elif index == 2:
            key = StringSlice("RTK_AUDIT_DIR")
        js_literal(sink, StringSlice('set "') if windows else StringSlice("export "))
        js_literal(sink, key)
        js_byte(sink, 61)
        if windows:
            rtkw_escape(sink, states[unsafe_offset=index], True)
            js_literal(sink, StringSlice('"\r\n'))
        else:
            rtkw_quote(sink, states[unsafe_offset=index], False)
            js_byte(sink, 10)


def rtkw_unavailable(sink: Pointer[mut=True, JsonSink, _], command: ProdexRichStringView, windows: Bool):
    if windows:
        js_literal(sink, StringSlice("@echo off\r\n>&2 echo prodex: "))
        rtkw_escape(sink, command, True)
        js_literal(sink, StringSlice(" requested but '"))
        rtkw_escape(sink, command, True)
        js_literal(sink, StringSlice("' is not installed or not on PATH\r\nexit /b 127\r\n"))
    else:
        js_literal(sink, StringSlice("#!/usr/bin/env sh\necho 'prodex: "))
        # The complete diagnostic is single-quoted, including its literal apostrophes.
        rtkw_escape(sink, command, False)
        js_literal(sink, StringSlice(" requested but '\\''"))
        rtkw_escape(sink, command, False)
        js_literal(sink, StringSlice("'\\'' is not installed or not on PATH' >&2\nexit 127\n"))


def rtkw_emit(
    sink: Pointer[mut=True, JsonSink, _], windows: Bool, kind: Int64,
    texts: Pointer[mut=False, ProdexRichStringView, _],
    arguments: Pointer[mut=False, ProdexRichStringView, _], count: Int64,
    states: Pointer[mut=False, ProdexRichStringView, _], state_count: Int64,
):
    var rtk = texts[unsafe_offset=0].copy()
    var command = texts[unsafe_offset=1].copy()
    if kind == 2:
        rtkw_unavailable(sink, command, windows)
        return
    if kind == 1:
        js_literal(sink, StringSlice("@echo off\r\n") if windows else StringSlice("#!/usr/bin/env sh\n"))
        rtkw_states(sink, windows, states, state_count)
        if not windows:
            js_literal(sink, StringSlice("exec "))
        rtkw_quote(sink, command, windows)
        for index in range(count):
            js_byte(sink, 32)
            rtkw_quote(sink, arguments[unsafe_offset=index], windows)
        js_literal(sink, StringSlice(" %*\r\nexit /b %ERRORLEVEL%\r\n") if windows else StringSlice(' "$@"\n'))
        return
    if windows:
        js_literal(sink, StringSlice('@echo off\r\nif "%PRODEX_RTK_DISABLE_AUTO_WRAP%"=="1" goto prodex_rtk_raw\r\nif defined PRODEX_RTK_AUTO_WRAP_DEPTH goto prodex_rtk_raw\r\n'))
        if count > 0:
            js_literal(sink, StringSlice(':prodex_rtk_scan\r\nif "%~1"=="" goto prodex_rtk_raw\r\n'))
            for index in range(count):
                js_literal(sink, StringSlice('if /I "%~1"=="'))
                rtkw_escape(sink, arguments[unsafe_offset=index], True)
                js_literal(sink, StringSlice('" goto prodex_rtk_wrap\r\n'))
            js_literal(sink, StringSlice('shift\r\ngoto prodex_rtk_scan\r\n:prodex_rtk_wrap\r\n'))
        js_literal(sink, StringSlice('set "PRODEX_RTK_AUTO_WRAP_DEPTH=1"\r\n'))
        rtkw_states(sink, True, states, state_count)
        rtkw_quote(sink, rtk, True)
        js_byte(sink, 32)
        rtkw_quote(sink, command, True)
        js_literal(sink, StringSlice(' %*\r\nexit /b %ERRORLEVEL%\r\n:prodex_rtk_raw\r\n'))
        rtkw_quote(sink, command, True)
        js_literal(sink, StringSlice(' %*\r\nexit /b %ERRORLEVEL%\r\n'))
        return
    js_literal(sink, StringSlice('#!/usr/bin/env sh\nif [ "${PRODEX_RTK_DISABLE_AUTO_WRAP:-}" = "1" ] || [ -n "${PRODEX_RTK_AUTO_WRAP_DEPTH:-}" ]; then\n  exec '))
    rtkw_quote(sink, command, False)
    js_literal(sink, StringSlice(' "$@"\nfi\n'))
    if count > 0:
        js_literal(sink, StringSlice('for prodex_rtk_arg in "$@"; do\n  case "$prodex_rtk_arg" in\n    '))
        for index in range(count):
            if index > 0:
                js_byte(sink, 124)
            var argument = arguments[unsafe_offset=index].copy()
            var source = rich_view_ptr(argument)
            for offset in range(Int64(argument.len)):
                var byte = source[unsafe_offset=offset]
                if byte == 124:
                    js_byte(sink, 92)
                js_byte(sink, byte)
        js_literal(sink, StringSlice(')\n      export PRODEX_RTK_AUTO_WRAP_DEPTH=1\n      '))
    else:
        js_literal(sink, StringSlice('export PRODEX_RTK_AUTO_WRAP_DEPTH=1\n'))
    rtkw_states(sink, False, states, state_count)
    js_literal(sink, StringSlice('exec '))
    rtkw_quote(sink, rtk, False)
    js_byte(sink, 32)
    rtkw_quote(sink, command, False)
    js_literal(sink, StringSlice(' "$@"\n'))
    if count > 0:
        js_literal(sink, StringSlice('      ;;\n  esac\ndone\nexec '))
        rtkw_quote(sink, command, False)
        js_literal(sink, StringSlice(' "$@"\n'))


@export("prodex_rtk_wrapper_script_v1")
def prodex_rtk_wrapper_script_v1(
    abi_version: Int64, platform: Int64, kind: Int64,
    texts_address: UInt, arguments_address: UInt, argument_count: Int64,
    states_address: UInt, state_count: Int64,
    measuring: Int64, output_address: UInt, capacity: Int64, metadata_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (
        platform < 0 or platform > 1 or kind < 0 or kind > 2
        or texts_address == 0 or texts_address % 8 != 0
        or argument_count < 0 or argument_count > RTKW_MAX_ARGUMENTS
        or (argument_count > 0 and (arguments_address == 0 or arguments_address % 8 != 0))
        or (state_count != 0 and state_count != 3)
        or (state_count > 0 and (states_address == 0 or states_address % 8 != 0))
        or measuring < 0 or measuring > 1 or capacity < 0
        or metadata_address == 0 or metadata_address % 8 != 0
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and output_address == 0)
    ):
        return 1
    var texts = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(texts_address))
    var arguments = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(arguments_address))
    var states = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(states_address))
    var total: Int64 = 0
    for group in range(3):
        var values = texts if group == 0 else (arguments if group == 1 else states)
        var count = Int64(2) if group == 0 else (argument_count if group == 1 else state_count)
        for index in range(count):
            var value = values[unsafe_offset=index].copy()
            if not rich_view_valid(value, RTKW_MAX_BYTES) or Int64(value.len) > RTKW_MAX_BYTES - total:
                return 1
            total += Int64(value.len)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var sink = JsonSink(output, capacity, 0, True, False)
    rtkw_emit(Pointer(to=sink), platform == 1, kind, texts, arguments, argument_count, states, state_count)
    if sink.failed:
        return 3
    var required = sink.written
    if measuring == 0:
        if capacity < required:
            return 3
        sink = JsonSink(output, capacity, 0, False, False)
        rtkw_emit(Pointer(to=sink), platform == 1, kind, texts, arguments, argument_count, states, state_count)
        if sink.failed or sink.written != required:
            return 3
    var metadata = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(metadata_address))
    metadata[unsafe_offset=0] = 1
    metadata[unsafe_offset=1] = required
    return 0
