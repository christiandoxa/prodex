import { spawn } from "node:child_process";

export function normalizeGitPath(filePath) {
  return filePath.replaceAll("\\", "/").replace(/^\.\//, "");
}

export function formatCommand(command, args) {
  return [command, ...args].join(" ");
}

export function parsePositiveInteger(value, name) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}

export function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${formatCommand(command, args)} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(
          new Error(
            `${formatCommand(command, args)} exited with code ${code}${stderr.trim() ? `\n${stderr.trim()}` : ""}`,
          ),
        );
        return;
      }
      resolve({ stdout, stderr });
    });
  });
}

export async function git(args, options = {}) {
  return run("git", args, options);
}

export function globPatternToRegex(pattern) {
  let source = "^";
  for (let index = 0; index < pattern.length; index += 1) {
    const character = pattern[index];
    const next = pattern[index + 1];
    if (character === "*" && next === "*") {
      source += ".*";
      index += 1;
      continue;
    }
    if (character === "*") {
      source += "[^/]*";
      continue;
    }
    if ("\\^$+?.()|{}[]".includes(character)) {
      source += `\\${character}`;
      continue;
    }
    source += character;
  }
  source += "$";
  return new RegExp(source);
}

export function fileMatchesAnyPattern(filePath, patterns) {
  const normalized = normalizeGitPath(filePath);
  return patterns.some((pattern) => globPatternToRegex(pattern).test(normalized));
}

export function parseNumstat(output) {
  const rows = [];
  for (const line of output.split(/\r?\n/)) {
    if (!line.trim()) {
      continue;
    }

    const parts = line.split("\t");
    if (parts.length < 3) {
      continue;
    }

    const [insertionsRaw, deletionsRaw, ...pathParts] = parts;
    const filePath = normalizeGitPath(pathParts.join("\t"));
    const binary = insertionsRaw === "-" || deletionsRaw === "-";
    const insertions = binary ? 0 : Number(insertionsRaw);
    const deletions = binary ? 0 : Number(deletionsRaw);
    rows.push({
      filePath,
      insertions: Number.isFinite(insertions) ? insertions : 0,
      deletions: Number.isFinite(deletions) ? deletions : 0,
      binary,
    });
  }
  return rows;
}
