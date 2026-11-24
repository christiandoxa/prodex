#!/usr/bin/env node
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");
const postgresImage = "postgres:19.0-alpine@sha256:cac45fb483caee3668946088a1c0532db89d5285c28dbfeac889b4068296e06f";
const redisImage = "redis:8.12.0-alpine@sha256:ff55e25203f915f2fe9cc1428e6da969fec257dd6e6feb71a34fd0cf3a86b289";
export const MISSING_DEPENDENCY_MESSAGE =
  "ci:storage-postgres-proof requires either PRODEX_TEST_POSTGRES_URL or local docker + psql + openssl";

function assertSelfTest(condition, message) {
  if (!condition) {
    throw new Error(`self-test failed: ${message}`);
  }
}

export function selectProofMode({
  postgresUrl = process.env.PRODEX_TEST_POSTGRES_URL,
  dockerAvailable,
  psqlAvailable,
  opensslAvailable,
} = {}) {
  if (postgresUrl) {
    return "direct";
  }
  if (dockerAvailable && psqlAvailable && opensslAvailable) {
    return "managed";
  }
  throw new Error(MISSING_DEPENDENCY_MESSAGE);
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const env = { ...process.env, ...(options.env ?? {}) };
    for (const name of options.unsetEnv ?? []) delete env[name];
    const child = spawn(command, args, {
      cwd: repoRoot,
      env,
      timeout: options.timeout,
      stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
    });

    let stdout = "";
    let stderr = "";
    if (options.capture) {
      child.stdout.setEncoding("utf8");
      child.stderr.setEncoding("utf8");
      child.stdout.on("data", (chunk) => {
        stdout += chunk;
      });
      child.stderr.on("data", (chunk) => {
        stderr += chunk;
      });
    }

    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${command} ${args.join(" ")} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(
          new Error(
            `${command} ${args.join(" ")} exited with code ${code}${stderr.trim() ? `\n${stderr.trim()}` : ""}`,
          ),
        );
        return;
      }
      resolve({ stdout, stderr });
    });
  });
}

async function retry(action, attempts = 3, delayMs = 5000) {
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return await action();
    } catch (error) {
      if (attempt === attempts) {
        throw error;
      }
      await new Promise((resolve) => setTimeout(resolve, delayMs));
    }
  }
}

async function ensureDockerImage(image) {
  try {
    await run("docker", ["image", "inspect", image], { capture: true });
  } catch {
    await retry(() => run("docker", ["pull", image], { capture: true }));
  }
}

async function commandExists(command, args = ["--version"]) {
  try {
    await run(command, args, { capture: true });
    return true;
  } catch {
    return false;
  }
}

async function waitForPostgres(port) {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      await run(
        "psql",
        ["-h", "127.0.0.1", "-p", String(port), "-U", "postgres", "-d", "prodex_test", "-c", "select 1"],
        {
          capture: true,
          env: { PGPASSWORD: "postgres" },
        },
      );
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }
  throw new Error("temporary Postgres container did not become ready");
}

async function waitForRedis(containerId) {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      await run("docker", ["exec", containerId, "redis-cli", "ping"], { capture: true });
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
  }
  throw new Error("temporary Redis container did not become ready");
}

async function createTlsMaterial(tlsDir) {
  await fs.chmod(tlsDir, 0o755);
  const caKey = path.join(tlsDir, "ca.key");
  const caCert = path.join(tlsDir, "ca.crt");
  const serverKey = path.join(tlsDir, "server.key");
  const serverCsr = path.join(tlsDir, "server.csr");
  const serverCert = path.join(tlsDir, "server.crt");
  const extensions = path.join(tlsDir, "server.ext");
  await run(
    "openssl",
    [
      "req", "-x509", "-newkey", "rsa:2048", "-nodes",
      "-keyout", caKey, "-out", caCert, "-days", "1",
      "-subj", "/CN=Prodex Test CA",
      "-addext", "basicConstraints=critical,CA:TRUE",
      "-addext", "keyUsage=critical,keyCertSign,cRLSign",
    ],
    { capture: true },
  );
  await run(
    "openssl",
    [
      "req", "-new", "-newkey", "rsa:2048", "-nodes",
      "-keyout", serverKey, "-out", serverCsr, "-subj", "/CN=localhost",
    ],
    { capture: true },
  );
  await fs.writeFile(
    extensions,
    "subjectAltName=DNS:localhost\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n",
  );
  await run(
    "openssl",
    [
      "x509", "-req", "-in", serverCsr, "-CA", caCert, "-CAkey", caKey,
      "-CAcreateserial", "-out", serverCert, "-days", "1", "-extfile", extensions,
    ],
    { capture: true },
  );
  await fs.chmod(serverKey, 0o640);
}

async function verifyPostgresImageUpgrade() {
  // Resolve the maintained Compose mount and image with fixture-only secret paths.
  // No project service, named volume, or real database is started by this proof.
  const { stdout } = await run("docker", [
    "compose", "--env-file", "/dev/null", "--profile", "postgres",
    "--profile", "redis", "config", "--format", "json",
  ], {
    capture: true,
    env: {
      PRODEX_GATEWAY_TOKEN_FILE: "/dev/null",
      OPENAI_API_KEY_FILE: "/dev/null",
      PRODEX_GATEWAY_POSTGRES_URL_FILE: "/dev/null",
      PRODEX_GATEWAY_REDIS_URL_FILE: "/dev/null",
      PRODEX_POSTGRES_PASSWORD_FILE: "/dev/null",
    },
  });
  const service = JSON.parse(stdout).services.postgres;
  const volume = service.volumes.find((entry) => entry.type === "volume" && entry.source === "prodex-postgres");
  if (!volume || service.image !== postgresImage) {
    throw new Error("PostgreSQL proof must use the maintained Compose image and persistent volume");
  }
  await ensureDockerImage(service.image);
  const options = [
    "run", "--rm", "--network", "none", "--read-only",
    "--cap-drop", "ALL", "--security-opt", "no-new-privileges:true",
    "--user", service.user,
    "--tmpfs", `${volume.target}:rw,nosuid,nodev,uid=70,gid=70,mode=0700`,
    "--tmpfs", "/run/postgresql:rw,nosuid,nodev,uid=70,gid=70,mode=0700",
    "--tmpfs", "/tmp:rw,nosuid,nodev,mode=1777",
    "--env", `POSTGRES_VOLUME_ROOT=${volume.target}`,
    "--env", "POSTGRES_PASSWORD=prodex-disposable-image-proof",
  ];
  if (service.environment.PGDATA) options.push("--env", `PGDATA=${service.environment.PGDATA}`);
  const name = `prodex-pg-image-${process.pid}-${Date.now()}`;
  const legacyName = `${name}-legacy`;
  const freshName = `${name}-fresh`;
  try {
    await run("docker", [
      ...options, "--name", legacyName, "--entrypoint", "bash", service.image, "-ceu", String.raw`
  printf '16\n' > "$POSTGRES_VOLUME_ROOT/PG_VERSION"
  if docker-entrypoint.sh postgres > /tmp/legacy-startup.log 2>&1; then
    cat /tmp/legacy-startup.log >&2
    exit 1
  fi
  grep -F 'there appears to be PostgreSQL data in:' /tmp/legacy-startup.log
  grep -F "$POSTGRES_VOLUME_ROOT" /tmp/legacy-startup.log
  test "$(cat "$POSTGRES_VOLUME_ROOT/PG_VERSION")" = 16
  test ! -s "$PGDATA/PG_VERSION"
  printf 'Legacy volume refused before initialization\n'
  `,
    ], { capture: true, timeout: 90000 });
    await run("docker", [
      ...options, "--name", freshName, "--entrypoint", "bash", service.image, "-ceu", String.raw`
  case "$PGDATA" in
    "$POSTGRES_VOLUME_ROOT"/*) ;;
    *) printf 'PGDATA is outside the persistent volume\n' >&2; exit 1 ;;
  esac
  docker-entrypoint.sh postgres -c listen_addresses='' > /tmp/fresh-startup.log 2>&1 &
  server_pid=$!
  trap 'kill -INT "$server_pid" 2>/dev/null || :; wait "$server_pid" || :' EXIT
  ready=
  for attempt in {1..60}; do
    if grep -Fq 'PostgreSQL init process complete; ready for start up.' /tmp/fresh-startup.log && pg_isready -q; then
      ready=1
      break
    fi
    if ! kill -0 "$server_pid" 2>/dev/null; then break; fi
    sleep 1
  done
  if [ -z "$ready" ]; then cat /tmp/fresh-startup.log >&2; exit 1; fi
  test "$(cat "$PGDATA/PG_VERSION")" = "$PG_MAJOR"
  test "$(psql -U postgres -d postgres -Atc 'select 1')" = 1
  printf 'Fresh volume initializes inside the persistent mount\n'
  `,
    ], { capture: true, timeout: 90000 });
  } finally {
    for (const container of [legacyName, freshName]) {
      try {
        await run("docker", ["rm", "--force", "--volumes", container], { capture: true });
      } catch {}
    }
  }
  process.stdout.write("PostgreSQL image upgrade proof: legacy volume refused; fresh volume initialized safely\n");
}

async function runWithManagedPostgres() {
  selectProofMode({
    postgresUrl: null,
    dockerAvailable: await commandExists("docker"),
    psqlAvailable: await commandExists("psql"),
    opensslAvailable: await commandExists("openssl", ["version"]),
  });
  await verifyPostgresImageUpgrade();
  await ensureDockerImage(redisImage);

  const tlsParent = path.join(repoRoot, "target", "ci");
  await fs.mkdir(tlsParent, { recursive: true });
  const tlsDir = await fs.mkdtemp(path.join(tlsParent, "prodex-pg-tls-"));
  await createTlsMaterial(tlsDir);

  const containerName = `prodex-pg-proof-${process.pid}-${Date.now()}`;
  const { stdout: containerIdRaw } = await run(
    "docker",
    [
      "run",
      "-d",
      "--name",
      containerName,
      "-e",
      "POSTGRES_PASSWORD=postgres",
      "-e",
      "POSTGRES_DB=prodex_test",
      "-P",
      "-v",
      `${tlsDir}:/tls:ro`,
      "--entrypoint",
      "sh",
      postgresImage,
      "-c",
      "cp /tls/server.crt /tmp/server.crt && cp /tls/server.key /tmp/server.key && chown postgres:postgres /tmp/server.crt /tmp/server.key && chmod 600 /tmp/server.key && exec docker-entrypoint.sh postgres -c ssl=on -c ssl_cert_file=/tmp/server.crt -c ssl_key_file=/tmp/server.key",
    ],
    { capture: true },
  );
  const containerId = containerIdRaw.trim();
  let redisContainerId = null;
  try {
    const { stdout: redisContainerIdRaw } = await run(
      "docker",
      ["run", "-d", "--rm", "-P", redisImage],
      { capture: true },
    );
    redisContainerId = redisContainerIdRaw.trim();
    const { stdout: portRaw } = await run(
      "docker",
      ["port", containerId, "5432/tcp"],
      { capture: true },
    );
    const portText = portRaw.trim().split(":").at(-1);
    const port = Number.parseInt(portText ?? "", 10);
    if (!Number.isFinite(port) || port <= 0) {
      throw new Error(`failed to resolve temporary Postgres port from '${portRaw.trim()}'`);
    }
    try {
      await waitForPostgres(port);
    } catch (error) {
      const { stdout, stderr } = await run("docker", ["logs", containerId], { capture: true });
      throw new Error(`${error.message}\n${stderr || stdout}`);
    }
    const { stdout: redisPortRaw } = await run(
      "docker",
      ["port", redisContainerId, "6379/tcp"],
      { capture: true },
    );
    const redisPort = Number.parseInt(redisPortRaw.trim().split(":").at(-1) ?? "", 10);
    if (!Number.isFinite(redisPort) || redisPort <= 0) {
      throw new Error(`failed to resolve temporary Redis port from '${redisPortRaw.trim()}'`);
    }
    await waitForRedis(redisContainerId);
    await run("node", ["scripts/ci/storage-boundary-guard.mjs"], {
      env: {
        PRODEX_TEST_POSTGRES_URL: `postgres://postgres:postgres@127.0.0.1:${port}/prodex_test`,
        PRODEX_TEST_POSTGRES_TLS_URL: `postgres://postgres:postgres@localhost:${port}/prodex_test`,
        PRODEX_TEST_POSTGRES_TLS_CA: path.join(tlsDir, "ca.crt"),
        PRODEX_TEST_REDIS_URL: `redis://127.0.0.1:${redisPort}/0`,
      },
    });
    await run(
      "cargo",
      [
        "test", "--locked", "-q", "-p", "prodex-storage-postgres-runtime",
        "--test", "postgres_tls", "--", "--test-threads=1",
      ],
      {
        env: {
          PRODEX_TEST_POSTGRES_TLS_URL: `postgres://postgres:postgres@localhost:${port}/prodex_test`,
          PRODEX_TEST_POSTGRES_TLS_CA: path.join(tlsDir, "ca.crt"),
        },
      },
    );
  } finally {
    if (redisContainerId) {
      try {
        await run("docker", ["rm", "-f", "--volumes", redisContainerId], { capture: true });
      } catch {}
    }
    try {
      await run("docker", ["rm", "-f", "--volumes", containerId], { capture: true });
    } catch {}
    await fs.rm(tlsDir, { recursive: true, force: true });
  }
}

async function runSelfTest() {
  assertSelfTest(
    selectProofMode({
      postgresUrl: "postgres://example",
      dockerAvailable: false,
      psqlAvailable: false,
      opensslAvailable: false,
    }) === "direct",
    "explicit PRODEX_TEST_POSTGRES_URL must bypass local dependency checks",
  );
  assertSelfTest(
    selectProofMode({
      postgresUrl: "",
      dockerAvailable: true,
      psqlAvailable: true,
      opensslAvailable: true,
    }) === "managed",
    "local docker+psql availability must select managed Postgres mode",
  );
  let missingDependencyError = null;
  try {
    selectProofMode({
      postgresUrl: "",
      dockerAvailable: true,
      psqlAvailable: false,
      opensslAvailable: true,
    });
  } catch (error) {
    missingDependencyError = String(error);
  }
  assertSelfTest(
    missingDependencyError?.includes(MISSING_DEPENDENCY_MESSAGE),
    "missing local dependencies must fail with the stable helper message",
  );
  let retryAttempts = 0;
  await retry(
    async () => {
      retryAttempts += 1;
      if (retryAttempts < 3) {
        throw new Error("transient");
      }
    },
    3,
    0,
  );
  assertSelfTest(retryAttempts === 3, "transient image pulls must use the bounded retry budget");
  const { stdout: unsetRedisValue } = await run(
    process.execPath,
    ["-e", "process.stdout.write(process.env.PRODEX_TEST_REDIS_URL ?? 'unset')"],
    {
      capture: true,
      env: { PRODEX_TEST_REDIS_URL: "fixture-redis-url" },
      unsetEnv: ["PRODEX_TEST_REDIS_URL"],
    },
  );
  assertSelfTest(unsetRedisValue === "unset", "direct Postgres proof must clear inherited Redis test URLs");
}

export async function main() {
  if (process.argv.includes("--self-test")) {
    await runSelfTest();
    return;
  }
  if (process.argv.includes("--image-upgrade-check")) {
    await verifyPostgresImageUpgrade();
    return;
  }
  if (process.env.PRODEX_TEST_POSTGRES_URL) {
    await run("node", ["scripts/ci/storage-boundary-guard.mjs"], {
      unsetEnv: ["PRODEX_TEST_REDIS_URL"],
    });
    return;
  }
  await runWithManagedPostgres();
}

if (process.argv[1] && path.resolve(process.argv[1]) === modulePath) {
  main().catch((error) => {
    process.stderr.write(`storage-postgres-proof: ${error.stack ?? error.message}\n`);
    process.exitCode = 1;
  });
}
