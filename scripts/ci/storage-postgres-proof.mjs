#!/usr/bin/env node
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");
const postgresImage = "postgres:16-alpine";
const redisImage = "redis:7-alpine";
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
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: { ...process.env, ...(options.env ?? {}) },
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

async function runWithManagedPostgres() {
  selectProofMode({
    postgresUrl: null,
    dockerAvailable: await commandExists("docker"),
    psqlAvailable: await commandExists("psql"),
    opensslAvailable: await commandExists("openssl", ["version"]),
  });

  const tlsDir = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-pg-tls-"));
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
        "test", "-q", "-p", "prodex-storage-postgres-runtime",
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
        await run("docker", ["rm", "-f", redisContainerId], { capture: true });
      } catch {}
    }
    try {
      await run("docker", ["rm", "-f", containerId], { capture: true });
    } catch {}
    await fs.rm(tlsDir, { recursive: true, force: true });
  }
}

function runSelfTest() {
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
}

export async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  if (process.env.PRODEX_TEST_POSTGRES_URL) {
    await run("node", ["scripts/ci/storage-boundary-guard.mjs"]);
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
