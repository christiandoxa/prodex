# Local HTTP listener resilience

A native client can retry indefinitely without reaching profile selection
when the local proxy is no longer accepting connections. Retry or profile
rotation cannot repair a dead listening socket by itself.

## Failure and repair

Upstream tiny_http 0.12.0 terminates its internal accept thread after an
accept error and unwraps descriptor duplication for newly accepted
connections. Under temporary file-descriptor pressure either path can
leave a long-lived proxy handle without a working listener. Reproduction
on the original dependency triggered `EMFILE` in the stream-clone path,
reset the queued request and failed the same-port recovery test.

The audited local patch retries both OS operations while the server is
alive. It preserves the bound socket and the accepted stream, uses
10–250 ms backoff rather than spinning, and reports failures at powers
of two to bound diagnostic volume. No parsed HTTP request, submitted
upstream inference, SSE output or tool call is replayed by this loop.
The shutdown flag is checked between retry attempts.

Both errors are tested with real TCP requests in separate Linux test
processes limited to 96 descriptors. After releasing the pressure, the
original queued request and a second request on the same address succeed.
The user's process limits, active sessions and installed programs are not
changed. This proves recovery of the reproduced transport fault; it does
not assert that every outage, invalid profile or persistent resource
failure can be made successful.

## Third-party provenance

`vendor/tiny_http` contains runtime sources from the official 0.12.0 crate,
verified against its Cargo.lock archive SHA-256. Original MIT and Apache
licenses, crate metadata and unmodified source digests are retained.
`PRODEX-PATCH.json` identifies the two changed source files and the one
added retry helper. The preflight integrity guard checks every vendored
source hash and the actual Cargo patch selection, and a tamper test
rejects unrelated changes. No protocol, TLS or domain-policy fallback
was added. This is a Rust OS-I/O boundary repair, not duplicate domain
logic for an operation already owned by Mojo.

## Applying the release

Update Prodex normally and launch or resume using the saved session ID.
Existing processes are not hot-patched. Do not reconnect to an already
closed temporary `codex --remote` socket or delete persisted conversation
state in an attempt to repair a transport failure.
