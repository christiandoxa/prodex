# Runtime launch exit cancellation in Mojo

The compiled `prodex_launch_exit_status_cancelled_v1` policy now decides
whether a child exit must suppress automatic session recovery. It preserves
the existing exit-130, POSIX SIGINT, and Windows Ctrl-C precedence while Rust
keeps platform `ExitStatus` extraction, process supervision, and relaunch
effects.

Required-Mojo ABI and runtime-launch caller regressions cover cancellation,
non-cancellation, platform facts, and fail-closed ABI/input validation. There
is no Rust policy fallback.
