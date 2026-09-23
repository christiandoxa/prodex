#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
baseline="${root}/scripts/compat/upstream-baseline.json"
tunnel_source="${root}/crates/prodex-app/src/super_expose/openai_tunnel.rs"

codex_reference="$(jq -r '.codex.latestRelease.tag_name // empty' "${baseline}")"
codex_latest="$(gh api repos/openai/codex/releases/latest --jq '.tag_name')"
tunnel_reference="$(sed -n 's/^const OPENAI_TUNNEL_CLIENT_LATEST_STABLE_REFERENCE: &str = "\([^"]*\)";$/\1/p' "${tunnel_source}")"
tunnel_minimum="$(sed -n 's/^const OPENAI_TUNNEL_CLIENT_MINIMUM_VERSION: &str = "\([^"]*\)";$/\1/p' "${tunnel_source}")"
tunnel_latest_tag="$(gh api repos/openai/tunnel-client/releases/latest --jq '.tag_name')"
tunnel_latest="${tunnel_latest_tag#v}"

if [[ ! "${codex_reference}" =~ ^rust-v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "invalid Codex latest-stable reference: ${codex_reference}" >&2
  exit 1
fi
if [[ "${codex_latest}" != "${codex_reference}" ]]; then
  echo "Codex latest-stable reference drift: qualified=${codex_reference} upstream=${codex_latest}" >&2
  echo "Audit the new stable Codex release, refresh scripts/compat/upstream-baseline.json, then retry the release." >&2
  exit 1
fi
if [[ ! "${tunnel_minimum}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ || ! "${tunnel_reference}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "invalid tunnel-client version policy" >&2
  exit 1
fi
if [[ "${tunnel_latest}" != "${tunnel_reference}" ]]; then
  echo "tunnel-client latest-stable reference drift: qualified=${tunnel_reference} upstream=${tunnel_latest}" >&2
  echo "Qualify the new stable tunnel-client release and refresh OPENAI_TUNNEL_CLIENT_LATEST_STABLE_REFERENCE." >&2
  exit 1
fi

printf "codex\tminimum=0.153.2\treference=%s\tupstream=%s\tlatest\n" "${codex_reference#rust-v}" "${codex_latest#rust-v}"
printf "tunnel-client\tminimum=%s\treference=%s\tupstream=%s\tlatest\n" "${tunnel_minimum}" "${tunnel_reference}" "${tunnel_latest}"
