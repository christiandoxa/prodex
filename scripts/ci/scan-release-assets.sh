#!/usr/bin/env bash
set -euo pipefail

: "${CLAMAV_IMAGE:?set the pinned ClamAV image}"
scan_root="$(cd -- "${1:?provide the directory to scan}" && pwd -P)"
if [[ -z "$(find "$scan_root" -type f -print -quit)" ]]; then
  echo "release asset malware scan scope is empty" >&2
  exit 2
fi

docker pull "$CLAMAV_IMAGE"
docker image inspect "$CLAMAV_IMAGE" --format '{{range .RepoDigests}}{{println .}}{{end}}' \
  | grep -Fx "clamav/clamav@${CLAMAV_IMAGE##*@}"

database="$(docker volume create)"
scan_log="$(mktemp)"
trap 'unlink "$scan_log"; docker volume rm "$database" >/dev/null' EXIT
docker run --rm --mount "type=volume,source=$database,target=/var/lib/clamav" \
  --entrypoint freshclam "$CLAMAV_IMAGE" --stdout

scan_status=0
docker run --rm --network none --read-only --cap-drop ALL --security-opt no-new-privileges \
  --user "$(id -u):$(id -g)" --tmpfs /tmp:rw,nosuid,nodev,noexec \
  --mount "type=volume,source=$database,target=/var/lib/clamav,readonly" \
  --mount "type=bind,source=$scan_root,target=/scan,readonly" \
  --entrypoint clamscan "$CLAMAV_IMAGE" \
  --recursive --official-db-only=yes --fail-if-cvd-older-than=7 \
  --allmatch=yes --alert-exceeds-max=yes --max-filesize=100M --max-scansize=400M \
  --infected --no-summary --stdout /scan > "$scan_log" 2>&1 || scan_status=$?
cat "$scan_log"

# Each reviewed finding is bound to the release version and the full executable hash.
if [[ "$scan_status" == 1 \
      && "${scan_root##*/}" == release-assets \
      && "$(<"$scan_log")" == '/scan/prodex-x86_64-pc-windows-msvc.exe: Win.Trojan.Virut-225 FOUND' ]]; then
  checksum="$(sha256sum "$scan_root/prodex-x86_64-pc-windows-msvc.exe")"
  version="${PRODEX_RELEASE_SCAN_VERSION:-}"
  case "${version}:${checksum%% *}" in
    '0.171.2:10229478760ac554717473cf7771afb5f6c8fdbd83b66fb896a0e8cfd92bfc3f'|\
    '0.171.2:334d93fb3b399453221c61724c1112b8f6c069d5b57ea854505b1a0c0a120350'|\
    '0.171.3:65630e75517dde4bc94e35c042894885776eafc17e7b107ed369a27c099fdf73'|\
    '0.172.0:97a738a530dd51c0bd1ad2c1dfbaef8864a727f57a596ee850969b747a0f590c'|\
    '0.172.1:088d2186a231aecbef5481a756c16d07b049691f47dd251dc7e5da4e41e10aa6')
      echo "Reviewed ClamAV false positive for Prodex ${version} Windows x64 SHA-256 ${checksum%% *}."
      exit 0
      ;;
    *) echo "Unreviewed Prodex ${version} Windows x64 SHA-256 ${checksum%% *}." >&2 ;;
  esac
fi

exit "$scan_status"
