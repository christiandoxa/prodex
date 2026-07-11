# ADR 0300: Enterprise Boundary Guards in CI

## Status

Accepted

## Context

The enterprise refactor added focused guards for documentation, binary
boundaries, authentication, authorization, application/control-plane boundaries,
storage backends, gateway boundaries, observability, provider SPI, and
deployment security. Local preflight ran these guards, but the GitHub Actions
`process-guard` job still enforced only the older crate/domain guard subset.

That left the modular-monolith contracts easier to bypass in CI than in local
preflight.

## Decision

The CI `process-guard` job now runs the enterprise guard set after the existing
crate and domain checks. The testing guide lists the same guard commands so
local and CI validation have one visible contract.

## Consequences

- CI now protects enterprise boundary crates, deployment hardening, and docs
  evidence without waiting for a full Rust test lane.
- Local preflight and GitHub Actions enforce the same high-level enterprise
  guard surface.
- The new CI work is Node-only guard execution and does not add request-path
  runtime behavior.
