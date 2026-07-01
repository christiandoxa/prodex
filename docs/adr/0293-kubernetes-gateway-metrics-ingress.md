# ADR 0293: Kubernetes Gateway Metrics Ingress

## Status

Accepted.

## Context

The Kubernetes baseline includes a `ServiceMonitor` for
`/v1/prodex/gateway/metrics`, but the gateway NetworkPolicy only allowed ingress
from namespaces labeled as the application ingress tier. A Prometheus operator
running in a dedicated monitoring namespace would not have an explicit network
path to scrape metrics.

## Decision

Allow gateway ingress on port `4000` from namespaces labeled
`prodex.dev/network-tier=monitoring`.

The deployment security guard now rejects gateway NetworkPolicies that omit the
monitoring namespace selector or the metrics service port.

## Consequences

- Prometheus can scrape gateway metrics without broadening access to every
  namespace.
- Operators must label the monitoring namespace deliberately or replace the
  selector with their private monitoring policy.
- The metrics path remains protected by existing gateway/admin authentication
  and response-hardening behavior; this change only opens the Kubernetes network
  path for approved monitoring pods.
