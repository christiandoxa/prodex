# ADR 1069: Canonical Request Target and Plane Isolation

## Status

Accepted

## Context

The async compatibility front classified only a narrow path string and allowed every route except
`ControlPlane` on its data-plane listener. `Unknown` therefore reached the legacy backend, whose
independent suffix classifiers could reinterpret it as a provider route. Control mounts also
classified every subpath as control-plane traffic before the explicit operation planner ran.

The HTTP boundary additionally trimmed targets, discarded fragment text, and collapsed empty
segments in some planners. Classification and backend forwarding did not share one validated
object.

## Decision

`prodex-gateway-http` owns `CanonicalRequestTarget`. It accepts only bounded ASCII origin-form
targets and rejects whitespace/control bytes, fragments, literal backslashes, repeated slashes,
dot segments, malformed percent escapes, and percent-encoded delimiters. Its `Debug` output
redacts the target text.

The same object retains the exact path-and-query bytes and drives:

- explicit route classification;
- application HTTP/authentication planning; and
- async-front backend URI reconstruction.

The deployed legacy root gateway also constructs this object at its first request boundary. It
returns the same stable 400/404 failures before authentication, accounting, or provider work, then
passes the accepted exact path-and-query through its compatibility pipeline.

The typed data-plane inventory covers responses, compact, realtime, quota, chat completions,
embeddings, image generations/edits/variations, audio speech/transcriptions/translations, batch
collection/items, rerank, A2A, Anthropic messages, and model collection/items. Exact unversioned
compatibility aliases are listed deliberately. Health probes have a separate plane. Admin/SCIM
traffic becomes control-plane traffic only when an explicit control operation recognizes its
path; unknown control subpaths remain `Unknown`.

The async front uses positive plane allowlists:

- data-plane listener: typed data routes plus health;
- control-plane listener: typed control routes plus health; and
- `Unknown`: stable 404 without a backend request.

Absolute-form targets are rejected before forwarding. The front reconstructs the backend URI from
the validated target, preserving accepted query bytes exactly.

Runtime broker paths under `/__prodex/runtime/*` are not gateway routes. They remain on the
separate authenticated loopback broker listener and are intentionally rejected by both gateway
planes.

## Consequences

- Ambiguous request targets fail before authentication, accounting, audit cardinality, or backend
  work.
- Previously tolerated fragments, path normalization tricks, and arbitrary provider-route suffixes
  are intentional security compatibility breaks.
- Existing application planners can migrate incrementally because `classify_route` remains as a
  strict compatibility wrapper.
- Compile-time route aliases are tested in canonical/alias pairs and must resolve to the same kind
  and plane. Any future dynamic route publication must carry and validate an immutable target
  plane before activation.
- The async front alone is not the final architecture; later slices must retain this boundary while
  removing the duplicate policy hop.

## Verification

```bash
cargo test --locked -p prodex-gateway-http -p prodex-gateway-server -- --test-threads=1
cargo test --locked -p prodex-application -- --test-threads=1
cargo clippy --locked -p prodex-gateway-http -p prodex-gateway-server -p prodex-application --all-targets -- -D warnings
```
