# MCP Super Expose Origin admission policy migrated to compiled Mojo

The production HTTP MCP Origin admission now belongs to the existing compiled
Super Expose module, not the Rust request handler. The Rust boundary retains
RFC-compliant URL decoding via `url::Url`, and passes its parsed scheme, host,
port/userinfo/path/query/fragment facts plus the untouched request Origin and
Host to one versioned Mojo ABI. The compiled kernel decides the entire trust
policy: no Origin is permitted; when present, Unicode edge whitespace and
invalid URLs are denied; local HTTP is allowed only for a byte-exact
`http://127.0.0.1:<port>` matching Host; HTTPS is allowed only when the
casefolded URL host is the Host, `chatgpt.com`, or `chat.openai.com`, with no
explicit port. Both forms require no username/password, root-only path and
no query or fragment.

The duplicate Rust local-loopback, trusted-HTTPS and URL-constraint policy was
deleted without a Rust acceptance fallback. Rust now only obtains parsed
facts, invokes the typed ABI, and rejects invalid ABI output explicitly.
The compiled function validates all UTF-8 views, booleans and flag ranges
before returning its decision; it reuses Mojo Unicode trimming and an exact
ASCII casefold matching contract.

Direct ABI tests cover loopback, trusted domains, fake trusted suffixes,
header casing, Unicode padding, port, userinfo, query, fragment, path,
unknown/invalid URL and malformed ABI inputs. All 99 real app Super Expose
tests and 16 related Mojo-core tests pass. The no-fallback guard requires
production Mojo delegation and rejects restored Rust policy code.

This is a production-reachable security policy migration, not a waiver for
the independent long-term 75% Mojo production-share goal.
