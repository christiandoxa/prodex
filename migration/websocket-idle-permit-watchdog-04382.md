# WebSocket idle admission and precommit recovery qualification

## Incident evidence and limits

The October 17 report shows native WebSocket idle timeouts while remote
Responses V2 compaction runs. Saved Codex 0.163.0 logs show successful local
WebSocket handshakes and approximately five-minute native waits repeated
through each retry sequence. The quota display separately shows one ready
profile with a small positive weekly balance and two weekly-exhausted
profiles. Five-hour quota does not override an exhausted weekly window.

The exact affected upstream broker logs were not available after those
sessions exited. The client evidence does not by itself prove every upstream
stall has the same cause. Two independent source defects were therefore
reproduced against the unmodified baseline before patching, using synthetic
loopback servers and no credential-bearing provider requests.

## Reproduced defects

1. A completed Responses connection retained its weighted profile permit in
   `RuntimeWebsocketSessionState`. Native clients cache these connections;
   enough idle sockets could make the only ready profile look saturated.
   The original fan-out regression closed every client immediately and did
   not exercise this condition. The new regression observed a retained
   count of two where an idle completed connection must have count zero.
2. Selection checked a request-scoped deadline, but the synchronous upstream
   frame loop did not. Text prelude receipt reset its socket idle timeout,
   so hidden progress could outlive the request watchdog and ultimately
   produce a retryable account transport failure. The new regression failed
   because an expired logical request did not return the typed local timeout.

## Patch boundaries

Response completion releases the response permit, not the reusable socket or
its response/turn-state lineage. Reuse reacquires active execution capacity.
Realtime duplex still owns a live permit. Existing compiled Mojo quota,
selection, affinity and retry decisions remain authoritative: this patch
changes host resource lifetime and applies the existing monotonic deadline
at the synchronous I/O boundary, without a duplicate policy implementation.

The frame loop bounds existing read/write timeouts and checks the request
watchdog before and after reads. Expiry resets only the failed upstream
attempt and flows through the existing `504 request_timeout` message handler.
It does not penalize account health or turn transport trouble into quota.
After commitment, normal idle timeout and no-replay semantics remain intact.

## Reproduction and checks

Run the real, compiled-Mojo caller tests with:

```sh
PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib recovery_lifecycle -- --test-threads=1
PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib runtime_proxy_idle_websockets_leave_capacity -- --test-threads=1
PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-app --lib runtime_proxy::websocket -- --test-threads=1
```

The first two regression cases failed before the production change and passed
afterward. Further tests guard committed output, realtime lifetime, repeated
prelude metadata, same-local-socket recovery and compaction with multiple idle
clients. All fixtures use loopback endpoints, synthetic request IDs and
placeholder accounts; no private incident identifiers are stored here.
