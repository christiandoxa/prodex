# Hardening Risk And Rollback

Rollback must preserve profile affinity, active stream ownership, user-managed
tool installations, and scoped Smart Context state. Do not delete user data or
downgrade persisted schemas in place.

| Severity | Risk and detection | Control | Rollback | Owner/action |
| --- | --- | --- | --- | --- |
| Medium | Caveman is missing or its manifest, version, digest, path, or entry point is invalid; doctor reports `missing`, `invalid`, or `degraded`. | Resolution is side-effect-free; normal Super skips optional tools; `--require-tool caveman` fails before TUI launch. | Remove Caveman from the selected/required tool set or restore the vetted user-managed installation. Prodex never deletes it. | Runtime launch: remove the one-release unversioned fallback in 0.348.0. |
| Medium | A compatibility installation resolves differently after the fallback window. | The canonical path is `<managed-root>/caveman/<version>/`; canonicalized paths and exact provenance are required. | Pin the documented versioned directory before upgrading; roll back the Prodex binary without modifying the installation. | Release engineering: publish the fallback removal in migration notes. |
| High | A Smart Context store is corrupt, wrong-scope, expired, or cannot be decrypted. | Reads verify schema, scope, digest, byte length, exact content, retention, and permissions; invalid stores are quarantined and health becomes degraded. | Set `PRODEX_SMART_CONTEXT_CANARY_PERCENT=0`, preserve the quarantined file for diagnosis, and start with a new scoped store. Never copy a store into another scope. | Runtime proxy: retain corruption, scope-isolation, and restart tests. |
| Medium | An older binary cannot read schema 3 or `sc2:`/`psc2:` state. | Legacy inputs are read-only; new writes use scoped schema 3 and strong digests. | Disable Smart Context before binary rollback. Keep the new store and key aside; do not rewrite it with the older binary. Restore the newer binary to reuse it. | Runtime proxy: remove legacy readers only after the stated compatibility window. |
| Medium | Active rewriting adds latency or a correctness signal trips. | 256 KiB HTTP/96 KiB WebSocket ceilings, 100 ms release deadline, positive tokenizer-counted margin, lossless expansion validation, panic cooldown, and whole-request fallback. | Set canary to 0 or send explicit exact mode. New requests become byte-identical pass-through; in-flight streams and affinity bindings are untouched. | Runtime proxy: monitor fallback reason and stage timing without source content. |
| Low | Shadow observation consumes excess CPU. | Only 1% of eligible shadow traffic is sampled; state is disposable and never committed. | Disable shadow or set canary to 0. | Runtime proxy: tune sampling only from measured evidence. |

Safe rollback order:

1. Stop admitting new Smart Context rewrites with the canary kill switch.
2. Let committed streams finish; do not move their continuation bindings.
3. Preserve scoped stores, encryption keys, runtime logs, and doctor output.
4. Roll back the binary or optional-tool selection.
5. Re-run `prodex capability super-doctor` and the Smart Context offline self-test
   before re-enabling either feature.

See [the 0.346 migration note](../migrations/0.346-optional-tools.md) for user
actions and compatibility dates.
