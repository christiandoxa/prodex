# ADR 0277: Application Provider Capability Composition

## Status

Accepted

## Context

Provider/model capability negotiation is already modeled in `prodex-domain` and
`prodex-provider-spi`, but application composition roots still needed a shared
use-case boundary before gateway admission builds a provider invocation. Without
that boundary, adapters can duplicate candidate selection or dispatch a route
that does not satisfy required capabilities such as streaming, tools, remote
compact, or websocket transport.

## Decision

Add `plan_application_provider_capability` to `prodex-application`.

The planner takes a `CapabilityRequest` plus provider route candidates, delegates
selection to `negotiate_provider_route_capability`, and returns only a
compatible provider plan. Unsupported or unavailable routes are returned through
`plan_application_provider_capability_error_response`, which reuses the stable
redacted capability error envelope and does not expose provider names, internal
model names, or missing capability internals.

## Consequences

Composition roots get one side-effect-free provider selection entry point before
reservation/admission dispatch. Provider fallback and model compatibility remain
transport-neutral and reusable without introducing HTTP frameworks, provider
SDKs, async runtimes, storage drivers, filesystem, or network access into the
application crate.
