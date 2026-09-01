# Prodex Expose

`prodex s expose` runs the Prodex Super runtime for the current workspace and
provides:

- a browser terminal;
- a Streamable HTTP MCP endpoint; and
- optionally, an external tunnel for the MCP endpoint and, in Cloudflare mode,
  the browser terminal.

In a real interactive terminal, the command opens the Expose mode picker before
starting an external tunnel. **Local only** is highlighted first. Explicit
Cloudflare provider flags bypass that picker; `--tunnel-provider openai` opens
the OpenAI setup screen in the same Ratatui terminal. A non-TTY invocation never
opens Ratatui and uses only explicit configuration.

The listener is loopback-bound. Expose grants Super-level authority as the
current operating-system user, so the URL is a capability, not a harmless
preview link. Anyone who obtains a complete public URL can use the authority
available to that expose process.

For the detailed mode and security model, use this document as the canonical
reference. The shorter [Expose reference](docs/expose.md) remains a maintained
summary.

## Quick start

Local-only browser and MCP (the safe default, including non-TTY use):

```bash
prodex s expose
```

Cloudflare Quick Tunnel (public browser terminal and public MCP):

```bash
prodex s expose --tunnel
```

The explicit equivalent is:

```bash
prodex s expose --tunnel-provider cloudflare-quick
```

Existing Cloudflare Tunnel (using detected or explicitly selected config):

```bash
prodex s expose --tunnel-provider cloudflare-existing
```

OpenAI Secure MCP Tunnel (local browser, remote MCP through OpenAI):

```bash
CONTROL_PLANE_TUNNEL_ID=tunnel_0123456789abcdefghijklmnopqrstuv \
CONTROL_PLANE_API_KEY='replace-with-your-runtime-key' \
prodex s expose --tunnel-provider openai
```

Replace the tunnel identifier and runtime key with values from the supported
OpenAI tunnel setup. The identifier must be `tunnel_` followed by 32 lowercase
letters or digits. The API key is deliberately not a command-line option.

## Interactive setup

With stdin and the terminal output attached to a TTY, the mode picker offers:

```text
Expose mode

> Local only
  Cloudflare Quick Tunnel
  Existing Cloudflare Tunnel
  OpenAI Secure MCP Tunnel
```

Use Up/Down to select, Enter to continue, and Esc or `q` to cancel. Cancellation
before startup has no external side effects. Prodex does not start
`cloudflared` or `tunnel-client` until an external mode has been explicitly
selected and confirmed. For OpenAI mode, the setup screen uses
`--openai-tunnel-id`, then `CONTROL_PLANE_TUNNEL_ID`, then a tunnel-ID field;
`CONTROL_PLANE_API_KEY` is used first and otherwise collected in a masked field.
Use Enter to advance, Backspace/Delete or Ctrl-U to edit, and Esc or Ctrl-C to
cancel. The Ready view keeps the selected mode visible.

## Mode comparison

| Mode | Browser terminal | MCP | External tunnel | Public browser shell |
|---|---|---|---|---|
| Local only | Local loopback | Local loopback | None | No |
| Cloudflare Quick Tunnel | Public through an ephemeral `trycloudflare.com` hostname | Public through the same Cloudflare endpoint | Prodex-supervised `cloudflared` Quick Tunnel | Yes |
| Existing Cloudflare Tunnel | Public through the configured existing hostname | Public through the same Cloudflare endpoint | A pre-created, user-managed Cloudflare tunnel/service | Yes |
| OpenAI Secure MCP Tunnel | Local loopback only | Remote through the supervised official tunnel client | OpenAI Secure MCP Tunnel | No |

OpenAI mode is MCP-only. It does not publish a generic browser reverse proxy,
does not emit a fabricated `openai.com` browser URL, and does not route browser
terminal traffic into an MCP transport. Cloudflare mode is the explicit public
browser-terminal mode.

## Command reference

The command is owned by the `ExposeArgs` parser. The current help surface is
available with:

```bash
prodex s expose --help
```

The principal syntax is:

```text
prodex s expose [OPTIONS] [CODEX_ARG]...
```

`[CODEX_ARG]...` is passed to Codex after Prodex's generated launch options.

### Expose options

| Option | Meaning |
|---|---|
| `--command <COMMAND>` | Shell command run inside the exposed PTY; defaults to `$SHELL` or `sh`. |
| `--cols <COLS>` | Initial terminal columns; default `100`. |
| `--rows <ROWS>` | Initial terminal rows; default `32`. |
| `--max-clients <MAX_CLIENTS>` | Concurrent browser clients, from 1 through 32; default `4`. |
| `--tunnel` | Explicitly selects the Cloudflare Quick Tunnel public expose path. It remains a boolean compatibility flag. |
| `--tunnel-provider cloudflare-quick` | Explicitly selects Cloudflare Quick Tunnel and bypasses the picker. `cloudflare` remains an alias. |
| `--tunnel-provider cloudflare-existing` | Explicitly selects a pre-created Cloudflare Tunnel from local config or a token file. `cloudflare-named` remains an alias. |
| `--tunnel-provider openai` | OpenAI Secure MCP Tunnel selection; browser remains local. |
| `--cloudflare-config <PATH>` | Existing Cloudflare config file; defaults to the official local search locations. Mutually exclusive with `--cloudflare-token-file`. |
| `--cloudflare-tunnel <NAME\|UUID>` | Existing Cloudflare tunnel name or UUID; otherwise read from the selected config. |
| `--cloudflare-hostname <HOSTNAME>` | Existing Cloudflare public hostname; otherwise use the unique loopback ingress hostname from config. |
| `--cloudflare-origin-port <PORT>` | Existing loopback origin port; otherwise use the matching config service port. |
| `--cloudflare-token-file <PATH>` | Existing Cloudflare token file passed to `cloudflared`; the token contents are never shown or accepted as an argv value. Mutually exclusive with `--cloudflare-config`. |
| `--openai-tunnel-id <ID>` | Non-secret OpenAI Platform tunnel identifier. It requires `--tunnel-provider openai`. |
| `--name <NAME>` | Suggested display name for the ChatGPT connection. |
| `-p, --profile <NAME>` | Starting Prodex profile; otherwise the active profile is used. |
| `--auto-rotate` | Allow eligible pre-commit profile rotation; this is the default. |
| `--no-auto-rotate` | Keep the selected profile fixed and fail rather than rotate. |
| `--auto-redeem` | Permit the existing guarded reset-credit redemption behavior. |
| `--skip-quota-check` | Skip the launch preflight quota gate. |
| `--full-access` | Compatibility flag; Super already launches with launch-time full access. |
| `--base-url <URL>` | Override the upstream ChatGPT base URL used by quota preflight and the runtime proxy. |
| `--no-proxy` | Disable system and environment proxy settings for upstream OpenAI/quota HTTP requests. |
| `--presidio` / `--no-presidio` | Enable or disable request-body and WebSocket redaction without prompting. |
| `--sub-agent` / `--no-sub-agent` | Enable or disable Codex sub-agent support. |
| `--sub-agent-provider <PROVIDER>` | Provider used by sub-agents. Detail flags require explicit sub-agent selection. |
| `--sub-agent-model <MODEL>` | Sub-agent model identifier. |
| `--sub-agent-model-reasoning-effort <EFFORT>` | Sub-agent reasoning effort. |
| `--sub-agent-url <URL>` | Local HTTP(S) endpoint for local sub-agents. |
| `--sub-agent-max-concurrency <VALUE>` | Active child process limit, from 1 through 64. |
| `--tool <TOOL>` / `--require-tool <TOOL>` | Add or require an optional Super tool. |
| `--url <URL>` | Route Codex directly to a local OpenAI-compatible `/v1` endpoint. |
| `--provider <PROVIDER>` | Select an external provider preset through Codex/Super. |
| `--harness <native\|minimal\|evaluated>` | Select the provider-bridge harness policy. |
| `--cli <CLI>` | Select a native agent CLI where supported, including `gemini`, `copilot`, `kiro`, or `agy`. |
| `--api-key <KEY>` | Provider API key option for provider bridges. Prefer the provider-specific environment variable so secrets do not enter shell history. |
| `--model <MODEL>` | Main model identifier; `--local-model` is an alias. |
| `--context-window <TOKENS>` | Context-window override for local/provider bridges. |
| `--auto-compact-token-limit <TOKENS>` | Auto-compaction threshold for local/provider bridges. |
| `--web-search <MODE>` | Hosted web-search mode: `disabled`, `cached`, `indexed`, or `live`. |
| `--dry-run` | Print the resolved mode, provider, local bind, binary, and redacted configuration source without starting Expose or a tunnel child. |
| `--no-tunnel` | Hidden deprecated compatibility alias for local-only behavior. |

Options inherited from Super are still subject to the selected provider's
validation and Codex's own option handling. `--tunnel` cannot be combined with
`--tunnel-provider` or `--no-tunnel`.

### Backwards compatibility

These meanings are preserved:

```bash
prodex s expose                 # local-only by default
prodex s expose --tunnel        # Cloudflare public expose path
prodex s expose --no-tunnel     # hidden local-only compatibility alias
```

`--tunnel` remains the Quick Tunnel compatibility flag and is never an alias
for Existing Cloudflare or OpenAI mode. The explicit provider values are
`cloudflare-quick` (alias `cloudflare`), `cloudflare-existing` (alias
`cloudflare-named`), and `openai`.

The interactive picker is not used when stdin or the terminal output is not a
TTY. In that case, bare `prodex s expose` remains local-only; use `--tunnel` or
an explicit supported provider flag in scripts when external access is intended.

## Architecture

The runtime has one owner process and one isolated expose instance:

```text
Prodex Super parent
  ├─ loopback HTTP server (127.0.0.1:0 for local/Quick/OpenAI)
  │    ├─ browser terminal routes (/expose, session, input, stream)
  │    └─ MCP route (/pdx/v1/<capability>/mcp)
  ├─ Super PTY and bounded run manager
  └─ optional supervised external child
       ├─ cloudflared (Quick or Existing Cloudflare mode)
       └─ official tunnel-client (OpenAI MCP mode)
```

The current working directory is captured as workspace context. It is not a
security sandbox: the Super child retains the launch-time authority of the
current OS user.

The MCP endpoint supports JSON responses for `server/discover`, `initialize`,
`ping`, `tools/list`, and `tools/call`. Expose does not require a long-lived
GET/SSE MCP endpoint; its readiness probes require JSON protocol responses.
Notifications are accepted with `202 Accepted` and no response body.

The endpoint exposes bounded Super operations rather than an arbitrary shell
MCP primitive:

- `prodex_super_start`
- `prodex_super_status`
- `prodex_super_events`
- `prodex_super_result`
- `prodex_super_cancel`
- `prodex_super_list`

The run manager permits four active runs, sixteen queued runs, thirty-two
retained terminal runs, 256 events per run, 8 KiB per event, and 256 KiB of
final output. Run task text is carried through bounded private task state and
the normal Prodex child launch path.

## Startup and readiness

The lifecycle is:

1. Parse Super and Expose options and validate provider URLs/options.
2. Resolve the workspace, profile, provider, model, effort, sub-agent setup,
   and remembered preferences. Interactive tunnel launches freeze the confirmed
   configuration for this process.
3. Create a fresh capability and bind the loopback listener.
4. Start the Super PTY and local HTTP server.
5. Probe local MCP `initialize`, then `tools/list`.
6. If requested, start the selected Quick/Existing Cloudflare or OpenAI child.
7. Probe the public Cloudflare MCP endpoint when a Cloudflare route is selected,
   or validate the tunnel-client's local health endpoints in OpenAI mode.
8. Print the ready status and keep the parent, child runs, listener, and tunnel
   supervised until cancellation, normal shutdown, or a real child failure.

Local readiness means local MCP `initialize` and `tools/list` passed. Cloudflare
readiness additionally means the public endpoint passed those probes. OpenAI
readiness means the local MCP passed and the tunnel-client child remained alive
after its local `/healthz` and `/readyz` checks succeeded.

The existence of a tunnel child alone is not readiness. A child that exits
after readiness causes Expose to fail closed with a tunnel-unavailable error.

## Local mode

`prodex s expose` binds the origin to `127.0.0.1:0`, so the operating system
chooses a free loopback port. Prodex prints:

- a local browser URL containing a one-time bootstrap fragment; and
- a local MCP URL containing the capability path.

The browser exchanges its bootstrap value for an HttpOnly, SameSite session
cookie. Browser input requires the session and CSRF checks. The browser stream
uses a bounded SSE presentation channel, while MCP uses the JSON endpoint.

Local mode has no external tunnel and requires no inbound firewall exposure.
Ctrl+C stops the server, PTY, run manager, and child process trees.

## Cloudflare Quick Tunnel

Quick Tunnel mode uses the installed `cloudflared` executable. Prodex checks
`cloudflared --version` and its tunnel help before starting. It creates a
private temporary configuration area and removes it during shutdown.

The managed command is equivalent to:

```text
cloudflared --config <private-config> tunnel --no-autoupdate --protocol auto --url http://127.0.0.1:<origin-port>
```

Prodex does not require a Cloudflare account, login, DNS record, `init`, or a
user-authored tunnel configuration for Quick Tunnel mode. It accepts only a
strict HTTPS `*.trycloudflare.com` hostname from the child output.

The discovered hostname is allowed for the browser and MCP routes. The public
browser URL uses the same one-time bootstrap fragment as the local browser URL:

```text
https://<hostname>/expose#bootstrap=<opaque-bootstrap>
```

The public MCP URL is:

```text
https://<hostname>/pdx/v1/<opaque-capability>/mcp
```

The top-level legacy `prodex expose --tunnel` command is a separate browser
terminal path with its own lifecycle; the `prodex s expose` Cloudflare path is
the documented Super mode here.

### QUIC to HTTP/2 fallback

Quick Tunnel starts with `--protocol auto`. The expected transport order is:

```text
QUIC / UDP 7844
        ↓ if negotiation does not complete
HTTP/2 / TCP 7844
```

Transport negotiation is bounded. When auto negotiation reports a hostname but
does not register a transport before its deadline, Prodex stops that child and
makes one explicit HTTP/2 attempt. This is a transport fallback, not a silent
provider switch and not a readiness claim.

The known failure class is a network that blocks UDP/7844 while permitting
TCP/7844. In that case QUIC can fail while HTTP/2 registers successfully.
Cloudflare management/update traffic can also use TCP/443. The local origin
still remains loopback-bound.

### DNS, DoH, TLS, and MCP are separate layers

A registered Cloudflare transport does not prove that the public hostname is
already usable. For a newly allocated wildcard hostname, local DNS can lag.
Prodex first uses normal hostname resolution. For a `trycloudflare.com` host it
also has a bounded fallback lookup through:

```text
https://cloudflare-dns.com/dns-query
```

The resolved address is used only for that public probe while preserving the
hostname for TLS SNI and HTTP Host. It does not change the machine's resolver
configuration and does not send the capability to DNS.

These are distinct diagnostic layers:

```text
cloudflared transport registered
    ≠ public hostname resolves
    ≠ TCP/TLS connection succeeds
    ≠ MCP initialize succeeds
    ≠ MCP tools/list succeeds
```

If UDP/7844 is blocked, TCP/7844 works, the HTTP/2 tunnel registers, and both
local DNS and Cloudflare DoH are delayed or reset, Prodex must report bounded
public readiness failure. It does not pretend the public MCP endpoint is
ready. Use OpenAI mode only when its own network and authentication contract
is available; Prodex never silently switches providers.

### Existing Cloudflare Tunnel

This mode uses a pre-created, user-managed Cloudflare Tunnel. It is not a Quick
Tunnel and does not create a Cloudflare account resource, DNS record, tunnel, or
credential. Prodex starts `cloudflared tunnel run` with the existing tunnel
identity and supervises that child; it does not silently mutate the tunnel.

The interactive picker may preselect one valid detected configuration, but still
requires explicit confirmation. For deterministic use, select
`cloudflare-existing` with `--tunnel-provider`.

Prodex accepts either an official Cloudflare config file or a token file:

- `--cloudflare-config` (or `PRODEX_CLOUDFLARE_CONFIG`/`TUNNEL_CONFIG`) selects
  a config. Without an override, Prodex checks `~/.cloudflared/config.yml` and
  `config.yaml`, plus the standard Unix system locations when applicable.
- `--cloudflare-token-file` (or `PRODEX_CLOUDFLARE_TOKEN_FILE`/
  `TUNNEL_TOKEN_FILE`) selects a token file. Token mode also requires
  `--cloudflare-hostname` or `PRODEX_CLOUDFLARE_HOSTNAME`; the optional tunnel
  name/UUID and origin port can be supplied with their corresponding flags or
  environment variables.

A config must provide a tunnel name/UUID and exactly one usable loopback HTTP
ingress hostname, such as `prodex.example.com` mapped to
`http://127.0.0.1:<port>`. Use `--cloudflare-hostname` when multiple routes
exist. `--cloudflare-origin-port` must match the configured service port;
Prodex binds `127.0.0.1:<origin-port>` and does not rewrite ingress. The
configured hostname is preserved and used for the public browser and MCP
routes; Prodex never fabricates a hostname.

Config and token-file modes are mutually exclusive. Keep tunnel credentials and
token contents in the official Cloudflare/cloudflared files. Prodex never asks
for, displays, logs, or puts the token value in argv; it passes only the
selected file path to `cloudflared`. It does not create, delete, reconfigure, or
rotate the tunnel. If the ingress does not map the selected hostname to the
chosen loopback port, setup fails with an actionable error.

## OpenAI Secure MCP Tunnel

OpenAI mode uses the official [openai/tunnel-client](https://github.com/openai/tunnel-client)
as a supervised external process. Prodex does not implement or clone the
tunnel wire protocol. The integration points the client at the OpenAI control
plane and gives it the local MCP endpoint; the client owns the upstream tunnel
protocol and authentication behavior.

OpenAI readiness is layered. Local MCP initialization/tools, the tunnel-client
process, and its `/healthz`/`/readyz` responses prove local tunnel runtime
readiness only. They do not prove that ChatGPT can create a connector or that a
remote discovery/tool request has reached this process. Prodex therefore reports
the ChatGPT connector as **unverified** until an actual connector request is
observed; it does not fabricate a remote end-to-end probe.

The current integration requires the audited stable client `v0.0.13` at commit
`4b5267f823be0b046bb883aacb51603cfde3a0ea`. Install the supported client from
the official OpenAI tunnel surface, or set `PRODEX_TUNNEL_CLIENT_BIN` to an
explicit executable path:

```bash
PRODEX_TUNNEL_CLIENT_BIN=/absolute/path/to/tunnel-client \
CONTROL_PLANE_TUNNEL_ID=tunnel_0123456789abcdefghijklmnopqrstuv \
CONTROL_PLANE_API_KEY='replace-with-your-runtime-key' \
prodex s expose --tunnel-provider openai
```

The exact client command receives a private configuration path, the OpenAI
control-plane base URL `https://api.openai.com`, the validated tunnel ID, and
the API-key reference `env:CONTROL_PLANE_API_KEY`. It also receives loopback
health-listener and log-file paths. The runtime key is never placed in argv or
the generated configuration.

The generated client configuration binds the actual local MCP endpoint to the
explicit `main` channel. It does not use the browser URL. Control-plane polling
uses the official host-root API URL and the client-owned `/v1/tunnels/...`
routes; Prodex does not use a GET to `/v1/mcp/...` as a readiness probe.

OpenAI mode requires:

- a pre-created OpenAI Platform tunnel;
- `CONTROL_PLANE_API_KEY` in the environment; and
- the supported `tunnel-client` executable.

The tunnel identifier is non-secret routing/configuration data. The runtime key
is secret. Do not put it in shell history, a process argument, a committed file,
or a diagnostic log.

Prodex creates a private temporary directory containing the local MCP reference,
client configuration, health URL file, and client log path. It uses the
client's loopback `/healthz` and `/readyz` responses for startup readiness, then
checks that the child is still alive. The directory and child are removed or
terminated on normal exit, cancellation, startup failure, or post-ready child
failure.

The OpenAI tunnel path is independent of `trycloudflare.com`,
`cloudflare-dns.com`, `cloudflared`, and Cloudflare's UDP/7844 transport. The
Prodex-selected control-plane endpoint is `api.openai.com` over HTTPS/TCP 443;
the official client may have additional current network requirements, so follow
its own release documentation for deployment-specific restrictions.

When selected from the interactive picker, OpenAI setup shows the configured
non-secret tunnel ID and the fixed split: Browser **Local only**, MCP **OpenAI
Secure MCP Tunnel**. If the tunnel ID, runtime key, or supported client is
missing, setup stops with guidance instead of attempting a broken launch. The
runtime API key is never displayed.

The OpenAI Platform tunnel must be associated with the intended ChatGPT
workspace, and the relevant principals need Tunnels **Read** + **Use**. Seeing
or creating a tunnel is not proof that its runtime key or ChatGPT connector may
use it. A newly created tunnel can also need the documented propagation window
before connector setup sees it.

ChatGPT connector creation can fail before any command reaches the local MCP.
In that case inspect the tunnel-client status/log or its redacted support
archive and classify the failure as remote or permission/workspace related;
do not call local Prodex MCP unhealthy. Historical [issue #35](https://github.com/openai/tunnel-client/issues/35)
documents a ChatGPT-side SSE/404 probe, while the still-open [issue #41](https://github.com/openai/tunnel-client/issues/41)
documents a no-auth `server/discover` reconnect class. These upstream reports
are not proof that a current Prodex request failed locally.

## Credentials and security

Expose has two different capability surfaces:

- the local browser bootstrap and session cookie; and
- the MCP capability path in the printed MCP URL.

The MCP capability is generated from operating-system randomness, retained as a
digest for validation, and intended to be ephemeral. The complete URL is a
bearer credential. Treat copying, pasting, storing, and logging it as credential
handling. Stop Expose to revoke it; a new process creates a new capability.

This is not OAuth, account linking, identity authentication, or a multi-user
authorization system. Do not publish a public browser shell or MCP URL to an
untrusted audience. The URL grants the Super authority of the process, not a
restricted workspace-only permission set.

Prodex replaces only the selected profile's upstream auth headers in runtime
proxy paths. It keeps profile credentials isolated and redacts secret-like
diagnostic errors. Tunnel children receive only the environment/configuration
required by their provider; inherited Cloudflare and OpenAI tunnel settings are
removed before launch. Temporary tunnel directories are private and are
cleaned after their final consumer exits.

## Status and observability

After readiness, the non-TTY status output includes the instance, workspace,
provider, model, effort, local browser URL, tunnel provider, local MCP URL, and
the active lifetime/stop instructions. Cloudflare mode additionally prints the
public browser URL and ChatGPT MCP URL. OpenAI mode prints the validated tunnel
ID and safe client version, but no public browser URL.

The public Cloudflare browser URL contains the bootstrap capability in its
fragment, and the public MCP URL contains the MCP capability path. An OpenAI
tunnel ID is only an identifier and is never a browser URL.

With a TTY, the expose view shows lifecycle phases such as Preparing,
Cloudflare tunnel, OpenAI Secure MCP Tunnel, local MCP initialize/tools, public
MCP initialize/tools, Ready, Stopped, and Failed. State transitions are bounded
and failures are redacted before presentation.

For OpenAI mode, the Ready view means: local MCP ready, local browser on
loopback, and tunnel-client health ready. It also shows **ChatGPT connector:
not verified**. A remote `server/discover`, `initialize`, `tools/list`, or
`tools/call` observed by the tunnel runtime is a separate diagnostic result,
not an automatic consequence of `/readyz`.

### Complete URLs in the Ready view

The interactive Ready panel renders the complete value of every URL or
operational endpoint it shows. This includes `Public MCP URL`, `MCP URL`,
`Public Browser URL`, `Browser URL`, an Existing Cloudflare hostname, and any
OpenAI tunnel endpoint or identifier. Long values—including unbroken capability
URLs with no spaces—wrap at terminal display-column boundaries; they are never
right-clipped, ellipsized, or replaced with `...`.

Wrapping is presentation-only: the underlying URL and capability value remain
unchanged. Each wrapped field contributes its actual height, so later fields do
not overlap it. If a short terminal cannot show the whole status at once, the
central status body scrolls vertically while the header, footer, and full-access
warning remain stable and reachable. Resizing recomputes wrapping and clamps
the scroll position. OpenAI mode shows its local browser/MCP information and
safe tunnel identifier, but never invents a public browser URL.

## Process supervision and shutdown

The parent owns the HTTP server, PTY, run manager, and external tunnel child.
Cloudflare and OpenAI children are checked for real process exit; an absent
output line is not itself a ready or failed state. Ctrl+C or `q` in the expose
view cancels startup or shuts down a ready instance.

Shutdown terminates the relevant process tree, waits within the provider's
bounded cleanup period, stops the PTY/run manager, closes the listener, joins
output readers where applicable, and removes private temporary configuration.
An external child that dies after readiness is reported as unavailable rather
than automatically replaced with a different tunnel provider.

## Troubleshooting

Use this sequence before changing configuration:

```text
Did the local MCP initialize/tools/list probe pass?
  no  -> inspect the local listener, selected profile/provider, and local logs
  yes -> continue

Was Cloudflare transport registered?
  no  -> inspect cloudflared installation and UDP/TCP 7844 reachability
  yes -> continue

Does the public hostname resolve?
  no  -> local DNS or bounded Cloudflare DoH failure; wait/retry or use another mode
  yes -> continue

Does public TLS connect?
  no  -> public edge/TLS/network failure; do not treat transport registration as ready
  yes -> continue

Did public MCP initialize/tools/list pass?
  no  -> MCP route, Host/capability, or application readiness failure
  yes -> Cloudflare expose is ready
```

| Symptom | Layer | Safe action |
|---|---|---|
| `cloudflared` missing or version/help check fails | Local executable | Install a current `cloudflared` using Cloudflare's official instructions. Local mode does not need it. |
| QUIC negotiation times out | Cloudflare transport | Check outbound UDP/7844. Prodex makes one HTTP/2/TCP/7844 fallback when the auto attempt has a hostname but no registered transport. |
| HTTP/2 registers but `*.trycloudflare.com` does not resolve | DNS propagation | Treat transport and DNS separately. Wait within the bounded public probe, inspect local DNS, and check whether `cloudflare-dns.com` DoH is blocked/reset. |
| Public TLS or MCP probe times out | Public readiness | Check DNS, TCP/TLS, hostname, capability URL, and public route. Do not increase the timeout indefinitely or declare readiness manually. |
| `tunnel-client` missing | OpenAI executable | Install the audited official client or set `PRODEX_TUNNEL_CLIENT_BIN` to its absolute path. |
| OpenAI tunnel ID rejected | Configuration | Use `CONTROL_PLANE_TUNNEL_ID` or `--openai-tunnel-id` with `tunnel_` plus exactly 32 lowercase letters/digits. |
| OpenAI runtime key missing | Secret configuration | Set `CONTROL_PLANE_API_KEY` in the environment. There is no `--openai-api-key` option. |
| OpenAI tunnel does not become ready | OpenAI startup/network | Check the pre-created tunnel, outbound HTTPS/TCP 443 to the configured OpenAI control plane, client permissions, and the local MCP endpoint. The client health probe is bounded. |
| ChatGPT connector creation fails but `/readyz` is 200 | Remote connector/control plane | Check workspace association, Tunnels Read + Use, tunnel propagation, and whether any command appears in the tunnel-client log/support archive. `/readyz` does not prove ChatGPT connector readiness. |
| `LocalOriginPortInUse` | Local listener | Choose a different existing-tunnel origin port or stop the process that owns that exact loopback port. Quick/Local/OpenAI modes use an OS-selected port. |
| URL works locally but public browser route is 404 | Intentional routing or provider mode | Cloudflare mode publishes the browser route; OpenAI mode intentionally keeps it local. Use the local browser URL for OpenAI and inspect Host/capability routing for Cloudflare. |
| Tunnel child exits after Ready | Child lifecycle | Stop/restart Expose after inspecting the redacted status/log information. Prodex fails closed and does not silently switch providers. |
| Startup cancelled or Ctrl+C appears stuck | Cleanup | Allow the bounded child-tree cleanup to finish. Do not kill unrelated processes or delete active temporary state. |

For a machine that blocks UDP/7844 but permits TCP/7844, Cloudflare Quick
Tunnel can still work if HTTP/2 registers and the public DNS/TLS/MCP probes are
available. If Cloudflare wildcard DNS or its DoH fallback is blocked, OpenAI
Secure MCP Tunnel can be a better fit when its own TCP/443 and tunnel-auth
requirements are satisfied. It is not a general VPN and is not a replacement
for public browser access.

## Provider selection guidance

- Choose **Local only** when the browser and MCP client run on the same machine
  and no external access is needed.
- Choose **Cloudflare Quick Tunnel** for an ephemeral public browser terminal
  and public MCP endpoint without a Cloudflare account. Treat both complete
  URLs as full-access bearer credentials.
- Choose **Existing Cloudflare Tunnel** when a pre-created tunnel, stable
  hostname, and its official config or token file are already managed by the
  user. Treat both complete URLs as full-access bearer credentials.
- Choose **OpenAI Secure MCP Tunnel** when an OpenAI-supported surface needs
  remote MCP connectivity, while the browser terminal should remain local and
  no generic public browser shell is required.

Provider choice is explicit. A Cloudflare DNS failure does not silently select
OpenAI, and an OpenAI control-plane failure does not silently fall back to
Cloudflare.

## Failure semantics

- Local MCP startup failure: Expose cleans up and returns an error.
- Cloudflare transport, hostname, TLS, or public MCP readiness failure: Expose
  cleans up the listener/tunnel and returns a bounded error.
- OpenAI tunnel-client startup or health failure: Expose cleans up the local
  listener/client state and returns a bounded error.
- Tunnel child death after readiness: Expose fails closed; it does not create a
  replacement with another provider.
- User cancellation: Expose stops its owned children and returns cleanly.
- Local browser/session expiry or MCP capability mismatch: the endpoint rejects
  the request; it does not grant a new capability implicitly.

## Platform notes

The same Expose modes and loopback policy apply on Linux, macOS, and Windows.
The runtime uses platform-specific process-tree containment where available:
Unix children use private process groups, and Windows tunnel children can be
placed in a job object with kill-on-close behavior. Paths are passed as native
argument values, not through shell interpolation. Always validate the installed
`cloudflared`/`tunnel-client` executable on the target platform.

## Testing and source map

Offline focused tests use deterministic fake children and local HTTP fixtures;
they do not require a live Cloudflare account or live OpenAI tunnel:

```bash
cargo test --locked -q -p prodex-app --lib expose:: -- --test-threads=1
cargo test --locked -q -p prodex-cli --tests expose
npm run docs
```

The main implementation areas are:

- `crates/prodex-cli/src/runtime_args.rs` — Expose option ownership and tunnel
  provider enum;
- `crates/prodex-app/src/expose/super_expose.rs` — mode selection, lifecycle,
  listener, readiness sequencing, and shutdown;
- `crates/prodex-app/src/expose/runtime.rs` — local runtime and legacy expose
  plumbing;
- `crates/prodex-app/src/expose/runtime/cloudflared.rs` and
  `cloudflared_startup.rs` — isolated Cloudflare child and transport startup;
- `crates/prodex-app/src/expose/runtime/openai_tunnel.rs` — official
  tunnel-client supervision and local health readiness;
- `crates/prodex-app/src/expose/mcp/**` — MCP protocol, tools, and probes;
- `crates/prodex-app/src/expose/http.rs` and `routes.rs` — browser/session/MCP
  route boundaries;
- `crates/prodex-app/src/expose/run_manager/**` — bounded task lifecycle;
- `crates/prodex-app/src/expose/*_tests.rs` — deterministic capability,
  routing, Cloudflare, OpenAI tunnel, and lifecycle coverage.

The document is intentionally explicit about what Expose does not do: OpenAI
Secure MCP Tunnel is MCP connectivity, Cloudflare public readiness is checked
through separate transport/DNS/TLS/application layers, and the full-access
capability URLs are not a multi-user security boundary.
