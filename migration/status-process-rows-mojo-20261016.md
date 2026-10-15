# Status process-row parsing in Mojo

`prodex_status_process_rows_v1` now owns bounded `ps -Ao pid=,comm=,args=` line
acceptance, whitespace tokenization, and `u32` PID parsing. Rust still runs the
platform process probe, validates caller-owned span tables, materializes
`ProcessRow` strings, and applies canonical CLI command classification. The
compiled ABI skips malformed rows, rejects oversized snapshots, and keeps all
text and output buffers caller-owned.
