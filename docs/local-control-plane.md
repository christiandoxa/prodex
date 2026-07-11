# Prodex Local Control Plane

OpenCodex makes provider routing obvious by putting setup, provider health, model discovery, usage, and logs behind one local dashboard. Prodex already has stronger profile isolation, quota-aware rotation, continuation affinity, and runtime diagnostics; the gap is discoverability, not routing semantics.

Minimal first slice:

- keep `prodex dashboard` as static HTML plus small JSON endpoints;
- expose read-only provider presets, provider contracts, model catalog rows, profile/account summaries, quota summaries, and runtime/gateway pointers;
- make setup conservative: generate exact safe commands instead of writing provider secrets or mutating Codex config;
- show an empty-state onboarding path that works before any profile exists.

This keeps the dashboard a Codex-focused local control plane. Direct provider mutation can be added later behind the same API shapes, but masked secrets must never be accepted as real values.

## Gateway Route Workbench

The separate gateway admin dashboard at `/v1/prodex/gateway/admin` includes a responsive Route Workbench for authenticated `admin` and read-only `viewer` principals. It submits bounded JSON to `POST /v1/prodex/gateway/routes/explain` and renders the same pure route plan used by live gateway routing: alias resolution, token requirements, concrete candidates, typed rejection stages and reasons, output adjustments, warnings, and truncation state.

Explain is diagnostic only. It does not send an upstream request, refresh credentials, reserve quota or billing, increment request/model/inflight counters, claim circuit probes, mutate affinity or route rotation, trigger guardrail webhooks, or persist runtime state. The dashboard keeps the previous result visible while a new explanation loads, renders returned strings as text, and does not persist submitted request JSON. Explain results are on-demand and non-durable; Prodex does not add historical request-body or trace storage.
