# Prodex Local Control Plane

OpenCodex makes provider routing obvious by putting setup, provider health, model discovery, usage, and logs behind one local dashboard. Prodex already has stronger profile isolation, quota-aware rotation, continuation affinity, and runtime diagnostics; the gap is discoverability, not routing semantics.

Minimal first slice:

- keep `prodex dashboard` as static HTML plus small JSON endpoints;
- expose read-only provider presets, provider contracts, model catalog rows, profile/account summaries, quota summaries, and runtime/gateway pointers;
- make setup conservative: generate exact safe commands instead of writing provider secrets or mutating Codex config;
- show an empty-state onboarding path that works before any profile exists.

This keeps the dashboard a Codex-focused local control plane. Direct provider mutation can be added later behind the same API shapes, but masked secrets must never be accepted as real values.
