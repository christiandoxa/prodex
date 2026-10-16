# Anthropic model and credential attempt cursor migrated to Mojo

The existing compiled provider-constraints owner now initializes and advances
Anthropic model/credential retry indices via a versioned C ABI. When a model
fallback advances beyond the last model, the cursor is exhausted; rotating a
credential resets the model index to zero if another credential exists. A
missing cursor is terminal and cannot be restarted by a retry operation.

The Rust Anthropic transport now only passes counts and borrowed typed index
values to Mojo and consumes its checked result. Its old positive-count gate,
model increment, credential increment, model reset and boundary checks are
removed; there is no Rust retry cursor fallback. Upstream requests,
authentication, response handling, and logging remain host-side.

Direct compiled-Mojo ABI tests check initialization, exhaustion, six ordered
model/credential attempts, invalid operations, versions, pointer and index
bounds without output mutation. The three production caller tests verify
model exhaustion and credential rotation. The no-fallback guard protects the
caller's Mojo ownership and ABI contract.

This checkpoint does not claim the separate 75% project production-share
target has been reached.
