#!/usr/bin/env bash
# Shared diagnostics; the native harness validates source, artifact and nonempty selection.
set -euo pipefail
: "${PRODEX_FULL_TEST_SUITE:?full-test suite is required}"
case "${PRODEX_FULL_TEST_SUITE}" in
  *[!a-z0-9-]*|'') echo "invalid full-test suite" >&2; exit 2 ;;
esac
artifact_dir="${RUNNER_TEMP}/full-test-${PRODEX_FULL_TEST_SUITE}-diagnostics"
cargo_log="${artifact_dir}/cargo-test.log"
runtime_dir="${PRODEX_RUNTIME_LOG_DIR:-/tmp}"
runtime_pointer="${runtime_dir}/prodex-runtime-latest.path"
suite="${PRODEX_FULL_TEST_SUITE}"

case "${suite}" in
  workspace)
    command_description="node scripts/ci/full-rust-test.mjs --jobs 6 --test-threads 4 --timings --timings-json --no-prodex-app-lib (workspace passes --exclude prodex-app)"
    ;;
  remainder)
    command_description="prodex-app library remainder after the shared disjoint filters"
    ;;
  *)
    command_description="prodex-app filters: ${PRODEX_APP_FILTERS//$'\n'/, }"
    ;;
esac

rm -rf "${artifact_dir}"
mkdir -p "${artifact_dir}"

cat > "${artifact_dir}/metadata.txt" <<EOF
event_name=${GITHUB_EVENT_NAME}
ref=${GITHUB_REF}
sha=${GITHUB_SHA}
run_id=${GITHUB_RUN_ID}
run_attempt=${GITHUB_RUN_ATTEMPT}
runner_os=${RUNNER_OS}
suite=${suite}
command=${command_description}
started_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
EOF

rustc -Vv > "${artifact_dir}/rustc-version.txt"
cargo -Vv > "${artifact_dir}/cargo-version.txt"

set +e
status=0
case "${suite}" in
  workspace)
    node scripts/ci/full-rust-test.mjs \
      --jobs 6 \
      --test-threads 4 \
      --timings \
      --timings-json \
      --no-prodex-app-lib \
      > "${cargo_log}" 2>&1
    status=$?
    ;;
  *)
    node scripts/ci/native-test-artifact.mjs run .prodex-ci/full-app-tests > "${cargo_log}" 2>&1
    status=$?
    ;;
esac
set -e

if [ "${status}" -eq 0 ] && [ "${suite}" = "remainder" ] && \
   grep -Fq 'running 0 tests' "${cargo_log}"; then
  echo "prodex-app shard matched no tests" >> "${cargo_log}"
  status=1
fi

printf 'exit_code=%s\nfinished_at=%s\n' \
  "${status}" \
  "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  >> "${artifact_dir}/metadata.txt"

if [ "${status}" -eq 0 ]; then
  tail -n 8 "${cargo_log}"
  rm -rf "${artifact_dir}"
  exit 0
fi

tail -n 200 "${cargo_log}" > "${artifact_dir}/cargo-test-tail.txt"

if [ -f "${runtime_pointer}" ]; then
  cp "${runtime_pointer}" "${artifact_dir}/prodex-runtime-latest.path"
  latest_runtime_log="$(tr -d '\r' < "${runtime_pointer}")"
  printf 'runtime_log_pointer=%s\nruntime_log_target=%s\n' \
    "${runtime_pointer}" \
    "${latest_runtime_log}" \
    >> "${artifact_dir}/metadata.txt"
  if [ -n "${latest_runtime_log}" ] && [ -f "${latest_runtime_log}" ]; then
    cp "${latest_runtime_log}" "${artifact_dir}/$(basename "${latest_runtime_log}")"
    tail -n 200 "${latest_runtime_log}" > "${artifact_dir}/latest-runtime-log-tail.txt"
  fi
else
  printf 'runtime_log_pointer=%s\nruntime_log_target=missing\n' \
    "${runtime_pointer}" \
    >> "${artifact_dir}/metadata.txt"
fi

find "${runtime_dir}" -maxdepth 1 -type f \
  \( -name 'prodex-runtime-*.log' -o -name 'prodex-runtime-latest.path' \) \
  | sort > "${artifact_dir}/runtime-log-files.txt" || true

cargo run --locked -q --bin prodex -- doctor --runtime > "${artifact_dir}/doctor-runtime.txt" 2>&1 || true
cargo run --locked -q --bin prodex -- doctor --runtime --json > "${artifact_dir}/doctor-runtime.json" 2> "${artifact_dir}/doctor-runtime-json.stderr" || true

echo "Full test suite failed. Recent cargo output:"
cat "${artifact_dir}/cargo-test-tail.txt"

if [ -f "${artifact_dir}/latest-runtime-log-tail.txt" ]; then
  echo
  echo "Latest runtime log tail:"
  cat "${artifact_dir}/latest-runtime-log-tail.txt"
fi

exit "${status}"
