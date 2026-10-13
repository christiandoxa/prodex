# Capacity recovery after Responses keepalive events

## Read-only incident evidence

The affected BRP installation was running Prodex 0.437.8 and native Codex 0.162.1.
Investigation used existing session JSONL, a read-only SQLite connection, and authenticated
loopback GET requests to the runtime health and bounded log-snapshot endpoints. No BRP
configuration, source, process, profile, session, or installed binary was changed. No new
upstream model request was issued by the investigation.

On October 13, 2026, the affected HTTP Responses request recorded the following sequence
in local UTC+07:00 time:

- 10:43:37.740: the proxy was holding 79,420 pre-output bytes and remained uncommitted.
- 10:44:06.574: the proxy committed 79,555 bytes and one response ID.
- 10:44:06.681: native Codex logged an unhandled `keepalive` Responses event.
- 10:44:10.508: the persisted native task ended with `codex_error_info=server_overloaded`.

The terminal event contained no `status` field. That shape is already supported by the
native recovery classifier; it is not the cause of this precommit defect.

A separate preceding request did rotate through three eligible accounts. All three
returned overload. Available quota is not evidence of available model service capacity,
and this patch does not claim to eliminate upstream-wide saturation or override a chosen
model. A hard-bound earlier request also exhausted its bounded same-owner retry budget;
those ownership restrictions remain intentional.

## Reproduced local defect

The shared Mojo event classifier listed rate-limit and response-creation metadata as
precommit hold events, but omitted the exact `keepalive` event. As a result, both the HTTP
SSE inspection state machine and the WebSocket frame inspector considered the heartbeat a
commit-ready event. Once the local stream was marked committed, a subsequent overloaded
response correctly avoided replay, but the commitment itself had been premature.

The baseline production-boundary tests reproduce three independent failures: heartbeat-only
SSE commits instead of holding, heartbeat followed by overload becomes committed, and a
WebSocket heartbeat is not held. A negative test proves that actual text, tool-argument,
and reasoning-summary output stays committed even when followed by overload.

## Patch and safety contract

The existing compiled `prodex_runtime_websocket_event_kind_v1` Mojo owner now classifies
exact `keepalive` as precommit hold. There is no new Rust semantic fallback, no transport
or session restart, no synthetic `/goal resume`, and no change to context-window or
compaction settings. Unknown event names keep their existing conservative behavior.

Request-scoped deadlines, buffer limits, Retry-After handling, hard bindings, cancellation,
and post-commit no-replay guarantees are unchanged. Safe precommit retry stays within the
original request and existing account-selection policy. After genuine output has been
sent, the upstream error is preserved rather than duplicating side effects.

## Verification plan

The regression suite covers the public HTTP SSE and WebSocket inspectors, every byte-split
boundary of a synthetic heartbeat/failure sequence, actual-output negative controls, and
the async prefetch boundary. Local mock HTTP backends exercise successful rotation to a
second profile without leaking the failed attempt, including a missing Content-Type
header, and prove no replay after visible output. These integration cases are included
in the serialized runtime-test manifest.

All fixtures are synthetic. Private prompts, credentials, continuation tokens, account
identifiers, and raw user payloads are not copied into the repository. A restored-baseline
mutation must fail the new regression and then pass again after byte-exact restoration.
