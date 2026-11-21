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
trap 'docker volume rm "$database" >/dev/null' EXIT
docker run --rm --mount "type=volume,source=$database,target=/var/lib/clamav" \
  --entrypoint freshclam "$CLAMAV_IMAGE" --stdout

docker run --rm --network none --read-only --cap-drop ALL --security-opt no-new-privileges \
  --user "$(id -u):$(id -g)" --tmpfs /tmp:rw,nosuid,nodev,noexec \
  --mount "type=volume,source=$database,target=/var/lib/clamav,readonly" \
  --mount "type=bind,source=$scan_root,target=/scan,readonly" \
  --entrypoint clamscan "$CLAMAV_IMAGE" \
  --recursive --official-db-only=yes --fail-if-cvd-older-than=7 \
  --allmatch=yes --alert-exceeds-max=yes --max-filesize=100M --max-scansize=400M /scan
