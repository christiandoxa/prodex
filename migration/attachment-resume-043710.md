# Goal attachment persistence across overlay exit and resume

## Read-only incident evidence

Investigation of PDX running Prodex 0.437.9 and Codex 0.162.1 found 12 persisted
native goals. Ten referred to an overlay-local pasted attachment path; nine of
those overlays were missing while the matching shared attachment file still
existed. No inspected reference was missing at both locations. Only metadata,
existence checks and byte counts were inspected; private prompts, account data,
credentials, screenshots and original attachment payloads are not in this change.
The real user databases were not rewritten during investigation.

Native Codex materializes long pasted goal text as attachments/UUID/pasted-text-N.txt
and goal images as attachments/UUID/image-N.ext under its reported CODEX_HOME.
An oversized objective can be materialized again as goal-objective.md containing
those first-level references. Sharing the attachment directory preserves the files,
but does not canonicalize the absolute path embedded in the native goal database.
Deleting a temporary overlay therefore invalidates a reference even if its shared
file remains intact.

## Reproduced root causes

- In prodex-shared-codex-fs/src/prepare.rs, full/recent/exact maintenance previously
  required an exclusive global rollout lock before repairing goals. A different
  active child holding the shared lock made maintenance return without repairing
  the independent goal database. All existing live-rollout protection was correct,
  but placing goal repair behind it was not.
- Runtime home preparation disabled historical rollout maintenance and skipped
  goal repair too. In prodex-app's runtime_launch/resume_repair.rs, a picker resume
  had no selected ID and only repaired the rollout index, leaving stale goals.
- Goal SQLite updates only changed the outer reference string. Generated
  goal-objective.md contents still retained dead nested pasted text/image paths.
- An unconditional objective UPDATE could overwrite a concurrent user replacement.

Five actual production-API regressions fail on the baseline with the expected
assertion failures, not compilation errors. The patch restores the goals while
preserving foreign-thread goals and every guarded rollout byte.

## Repair and safety boundaries

Goal SQLite/file reference repair now runs independently before the exclusive
rollout maintenance lock. Exact-session repair reads at most 64 KiB from the
selected rollout metadata prefix to obtain its native root ID; it does not write
that rollout. Picker/fresh runtime preparation repairs stored goal references
without scanning historical rollouts. Existing file copy/metadata/no-follow checks
remain in force. Goal updates compare the original objective in the SQL WHERE
clause, leaving newer goals, status, budgets, counters and timestamps untouched.

The existing Mojo shared-attachment ABI owns a new exact generated-wrapper
classification. Only goal-objective.md under durable attachment storage may have
its generated references normalized. Raw pasted text and image files are opaque
and never rewritten. Wrapper reads are bounded, reject symlinks and outside-root
redirection, detect intervening edits, and use the existing atomic file writer.
Nested cyclic references cannot cause recursion because wrapper rewriting is one
bounded level, matching native materialization.

## Native cold-process verification

A synthetic test starts the installed native Codex app-server, creates a saved
thread, writes long text, a real PNG and a generated objective through fs/writeFile,
and stores a paused goal with a token budget using thread/goal/set. It closes the
process and removes only the first synthetic overlay. A second native process
returns the broken reference and its fs/readFile fails, reproducing the incident.

The production repair function is then called while an unrelated child shared
lock remains held. A third native process reads the canonical goal, its wrapper
and both nested attachments through native RPC. Their exact original bytes, paused
status and token budget are checked. The test sends no inference request and does
not read or modify any real user session. It proves persistence and native cold
readability, not a claim that every upstream model turn can never fail.

Additional regressions cover missing files, symlinked generated wrappers, raw text
containing path-like strings, compare-and-swap conflicts, independent thread
ownership, and idempotence. Deliberate source mutations must fail those production
boundary assertions and pass again after byte-exact restoration.
