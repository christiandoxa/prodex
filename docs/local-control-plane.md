# Prodex Local Control Plane

OpenCodex makes provider routing obvious by putting setup, provider health, model discovery, usage, and logs behind one local dashboard. Prodex adapts that interaction model while retaining its own profile isolation, quota-aware rotation, continuation affinity, runtime diagnostics, and proxy ownership. It does not run an OpenCodex daemon or let another process mutate Prodex/Codex profile state.

Current design:

- expose the static, dependency-free control center through `prodex gui`, `prodex s gui`, and the non-opening `prodex dashboard` form;
- open the native browser without a shell (`xdg-open` on Linux) and keep serving when browser launch fails;
- expose read-only provider presets, provider contracts, model catalog rows, profile/account summaries, quota summaries, and runtime/gateway pointers;
- expose only a bounded, path-validated, secret-redacted tail of the latest runtime log;
- render core local state before slower quota checks, serialize local API fetches for the synchronous HTTP server, avoid overlapping refreshes, and keep the previous snapshot visible;
- make setup conservative: generate exact safe commands instead of writing provider secrets or mutating Codex config;
- show an empty-state onboarding path that works before any profile exists.

This keeps the dashboard a Codex-focused local control plane. Direct provider mutation can be added later behind the same API shapes, but masked secrets must never be accepted as real values.

## Gateway Route Workbench

The separate gateway admin dashboard at `/v1/prodex/gateway/admin` includes a responsive Route Workbench for authenticated `admin` and read-only `viewer` principals. It submits bounded JSON to `POST /v1/prodex/gateway/routes/explain` and renders the same pure route plan used by live gateway routing: alias resolution, token requirements, concrete candidates, typed rejection stages and reasons, output adjustments, warnings, and truncation state.

Explain is diagnostic only. It does not send an upstream request, refresh credentials, reserve quota or billing, increment request/model/inflight counters, claim circuit probes, mutate affinity or route rotation, trigger guardrail webhooks, or persist runtime state. The dashboard keeps the previous result visible while a new explanation loads, renders returned strings as text, and does not persist submitted request JSON. Explain results are on-demand and non-durable; Prodex does not add historical request-body or trace storage.
