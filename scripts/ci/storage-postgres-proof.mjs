#!/usr/bin/env node
import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");
const postgresImage = "postgres:16-alpine";
const redisImage = "redis:7-alpine";
export const MISSING_DEPENDENCY_MESSAGE =
  "ci:storage-postgres-proof requires either PRODEX_TEST_POSTGRES_URL or local docker + psql";

function assertSelfTest(condition, message) {
  if (!condition) {
    throw new Error(`self-test failed: ${message}`);
  }
}

export function selectProofMode({
  postgresUrl = process.env.PRODEX_TEST_POSTGRES_URL,
  dockerAvailable,
  psqlAvailable,
} = {}) {
  if (postgresUrl) {
    return "direct";
  }
  if (dockerAvailable && psqlAvailable) {
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

async function runWithManagedPostgres() {
  selectProofMode({
    postgresUrl: null,
    dockerAvailable: await commandExists("docker"),
    psqlAvailable: await commandExists("psql"),
  });

  const containerName = `prodex-pg-proof-${process.pid}-${Date.now()}`;
  const { stdout: containerIdRaw } = await run(
    "docker",
    [
      "run",
      "-d",
      "--rm",
      "--name",
      containerName,
      "-e",
      "POSTGRES_PASSWORD=postgres",
      "-e",
      "POSTGRES_DB=prodex_test",
      "-P",
      postgresImage,
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
    await waitForPostgres(port);
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
        PRODEX_TEST_REDIS_URL: `redis://127.0.0.1:${redisPort}/0`,
      },
    });
  } finally {
    if (redisContainerId) {
      try {
        await run("docker", ["rm", "-f", redisContainerId], { capture: true });
      } catch {}
    }
    try {
      await run("docker", ["rm", "-f", containerId], { capture: true });
    } catch {}
  }
}

function runSelfTest() {
  assertSelfTest(
    selectProofMode({
      postgresUrl: "postgres://example",
      dockerAvailable: false,
      psqlAvailable: false,
    }) === "direct",
    "explicit PRODEX_TEST_POSTGRES_URL must bypass local dependency checks",
  );
  assertSelfTest(
    selectProofMode({
      postgresUrl: "",
      dockerAvailable: true,
      psqlAvailable: true,
    }) === "managed",
    "local docker+psql availability must select managed Postgres mode",
  );
  let missingDependencyError = null;
  try {
    selectProofMode({
      postgresUrl: "",
      dockerAvailable: true,
      psqlAvailable: false,
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
