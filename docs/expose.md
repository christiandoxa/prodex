# ChatGPT MCP expose

`prodex s expose` is a personal-development frontend for the real Prodex Super
runtime. It captures the current working directory, resolves the Super
configuration, starts the existing loopback expose listener, adds a focused
Streamable HTTP MCP route, starts `cloudflared tunnel --protocol http2 --url
http://127.0.0.1:<port>`, validates the reported Quick Tunnel hostname, and
probes the public endpoint before printing its URL.

## Modes

| Command | Behavior |
| --- | --- |
| `prodex expose` | Existing loopback browser terminal; no tunnel by default. |
| `prodex expose --tunnel` | Existing explicit public browser-terminal mode. |
| `prodex expose --no-tunnel` | Existing local-only compatibility mode. |
| `prodex s expose --no-tunnel` | Existing local-only browser-terminal mode. |
| `prodex s expose --tunnel` | Existing explicit public browser-terminal meaning. |
| `prodex s expose` | Super configuration followed by MCP-only Quick Tunnel mode. |

The public default does not publish `/expose`, `/input`, `/output`, `/stream`,
`/static`, or any other browser route. The loopback host keeps the browser
terminal and can be used for local protocol tests.

## Configuration

Interactive setup runs before capability generation or process startup:

1. main agent;
2. main model;
3. main reasoning effort, constrained by the selected model;
4. sub-agent enablement;
5. existing sub-agent provider/model/effort configuration when enabled;
6. resolved configuration summary.

The main and sub-agent model/effort choices use the same provider catalog,
effort metadata, validation, and picker implementation. Remembered preferences
seed a new interactive instance but remain editable. A running instance freezes
its confirmed configuration. A per-run MCP model or effort override applies only
to that run; null inherits the frozen instance default. Non-TTY execution does
not read stdin for configuration: explicit values, remembered values, and
normal Prodex defaults are resolved in that order.

## Authentication and transport

The URL has this form:

```text
https://<random>.trycloudflare.com/pdx/v1/<opaque-capability>/mcp
```

The capability is 32 bytes from the operating-system CSPRNG, encoded with
unpadded URL-safe base64. Prodex retains only its SHA-256 digest in the endpoint
object and compares incoming path segments with the existing constant-time
digest helper. It is never persisted, placed in `cloudflared` arguments or
environment, or included in routine diagnostics. Invalid, missing, malformed,
repeated, encoded, traversed, or revoked capability paths return 404.

The full URL is intentionally printed once after readiness. Treat it as a
credential: ChatGPT stores the connection URL, Cloudflare carries the path, and
anyone who obtains the complete URL receives the full capability granted by the
process. Stopping the process revokes access; restarting creates a new
capability and hostname. This is Ephemeral Capability Authentication, not OAuth,
account linking, identity authentication, or suitable authentication for a
public multi-user plugin.

The endpoint uses JSON responses for `server/discover`, legacy `initialize`,
`ping`, `tools/list`, and `tools/call`. It does not emit `text/event-stream` and
does not expose a long-lived GET/SSE endpoint. Notifications accepted by the
compatibility surface return `202 Accepted` with an empty body.

## Tools

The focused surface is:

- `prodex_super_start`: queues a full Super task and returns immediately with a `run_id`;
- `prodex_super_status`: reads one run state;
- `prodex_super_events`: reads a bounded monotonic event page;
- `prodex_super_result`: reads bounded final output and metadata;
- `prodex_super_cancel`: cancels one run and its complete child process tree;
- `prodex_super_list`: lists only runs owned by this expose process.

No shell-shaped MCP primitive is exposed. The run manager owns at most four
active runs, sixteen queued runs, thirty-two retained terminal runs, 256 events
per run, 8 KiB per event, and 256 KiB of final output. Run IDs are random
diagnostic identifiers, not credentials; every operation still requires the
outer capability.

Task text is sent to the Super child through stdin and never put in the child
argument vector. The child is the normal Prodex executable, launched with the
same Super flags, profile/provider behavior, model/effort settings, optional
tools, routing, auto-rotation, continuation policy, and workspace as local
`prodex s exec`. The MCP adapter owns ingress, schemas, and bounded lifecycle
state; Super remains the owner of execution semantics.

## Parallel workspaces

One expose process is the isolation unit. It owns one captured workspace, one
capability digest, one MCP identity, one tunnel, one run manager, and its own
child process groups. It binds to `127.0.0.1:0`, so multiple instances need no
manual port assignment and do not share a tunnel or kill group.

```bash
git worktree add ../feature-a -b feature/a
git worktree add ../feature-b -b feature/b
git worktree add ../feature-c -b feature/c

(cd ../feature-a && prodex s expose --name feature-a)
(cd ../feature-b && prodex s expose --name feature-b)
(cd ../feature-c && prodex s expose --name feature-c)
```

The MCP connection cannot select another workspace or run table. A run ID from
another instance is reported as unknown. Stopping one process revokes only its
capability, tunnel, listener, and children; other instances remain available.
Git worktrees use a `.git` file pointing at shared repository metadata, so Prodex
uses Git/current-working-directory behavior rather than assuming `.git/` is a
directory.

Prodex may intentionally share the established `PRODEX_HOME` profile, quota,
health, cooldown, auto-rotation, and remembered-preference state across
processes. Those writers use the existing merge-safe state model. Active expose
configuration and run state are not persisted or shared. Parallel pushes to
different branches are normal; concurrent conflicting Git writes remain real
Git conflicts.

## Cloudflare lifecycle

`cloudflared` is detected with `cloudflared --version` before Quick Tunnel
startup. Prodex invokes it directly with typed arguments, bounds output readers,
accepts only strict HTTPS `*.trycloudflare.com` hostnames, adds that exact Host
to the MCP-only route policy, and waits for a public MCP probe. The probe checks
modern `server/discover` where available (or legacy `initialize`) and then
`tools/list`. A hostname appearing in logs is not readiness.

No account, login, DNS record, OAuth page, or manually authored reverse-proxy
configuration is used. If `cloudflared` is missing, install it through the
official platform instructions; `--no-tunnel` remains available for local use.
If the child exits after readiness, Prodex fails closed and asks the user to
rerun rather than silently creating a new stale URL.

## Testing

The owning tests cover capability entropy/digest matching/redaction, malformed
paths and Host/Origin policy, JSON-only responses and notifications, browser
route isolation, bounded run state, stdin task transport, process-tree
cancellation, three concurrent managers, separate workspaces/sentinels, output
isolation, and stopping one instance without stopping the others. Live Cloudflare
and interactive ChatGPT Developer Mode checks remain environment-dependent and
must be reported as skipped when those services are unavailable.
