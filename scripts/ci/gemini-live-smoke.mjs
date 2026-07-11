#!/usr/bin/env node
import { spawn } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const DEFAULT_TIMEOUT_MS = 180_000;
const DEFAULT_EXTENDED_TIMEOUT_MS = 420_000;

function prodexBinary() {
  if (process.env.PRODEX_BIN) {
    return process.env.PRODEX_BIN.includes("/") ? resolve(process.env.PRODEX_BIN) : process.env.PRODEX_BIN;
  }
  return existsSync("target/debug/prodex") ? resolve("target/debug/prodex") : "prodex";
}

function baseArgs() {
  const args = ["s", "gemini", "--no-presidio"];
  if (process.env.PRODEX_LIVE_GEMINI_MODEL) {
    args.push("--model", process.env.PRODEX_LIVE_GEMINI_MODEL);
  }
  return args;
}

function simpleCommand(marker) {
  const args = baseArgs();
  args.push("exec", `Reply with exactly one line containing: ${marker}`);
  return { binary: prodexBinary(), args, cwd: process.cwd() };
}

function commandOutputOnlyCommand(marker) {
  const args = baseArgs();
  args.push(
    "exec",
    `Use the shell to run: printf ${JSON.stringify(marker)}. Then answer with only the command output.`,
  );
  return { binary: prodexBinary(), args, cwd: process.cwd() };
}

function extendedEditCommand(marker, workspace) {
  const args = baseArgs();
  args.push(
    "exec",
    [
      "In this workspace, create or overwrite gemini-smoke.txt with exactly these two lines:",
      `marker=${marker}`,
      "status=tool-edit-ok",
      "",
      "Then run this verification command from the workspace:",
      `node -e "const fs=require('fs'); const s=fs.readFileSync('gemini-smoke.txt','utf8'); if(!s.includes('status=tool-edit-ok')) process.exit(2); process.stdout.write('${marker}');"`,
      "",
      "Answer with only the command output.",
    ].join("\n"),
  );
  return { binary: prodexBinary(), args, cwd: workspace };
}

function extendedPatchCommand(marker, workspace) {
  const args = baseArgs();
  args.push(
    "exec",
    [
      `Create file gemini-patch-smoke.txt containing exactly ${marker} using apply_patch.`,
      "Then run cat gemini-patch-smoke.txt.",
      "Answer with only the command output.",
    ].join(" "),
  );
  return { binary: prodexBinary(), args, cwd: workspace };
}

function extendedReferenceCloneCommand(marker, workspace) {
  const args = baseArgs();
  const command = [
    "set -e",
    "rm -rf refs/gemini-cli refs/codex",
    "mkdir -p refs",
    "git clone -q --depth=1 https://github.com/google-gemini/gemini-cli.git refs/gemini-cli",
    "git clone -q --depth=1 https://github.com/openai/codex.git refs/codex",
    "test -d refs/gemini-cli/.git",
    "test -d refs/codex/.git",
    "grep -qi Gemini refs/gemini-cli/README.md",
    "grep -qi Codex refs/codex/README.md",
    `printf ${JSON.stringify(marker)}`,
  ].join(" && ");
  args.push(
    "exec",
    [
      "Use the shell to run exactly this verification command:",
      command,
      "",
      "If a command returns a running session id, poll or wait for it until the process exits before inspecting files.",
      "Do not stop after saying that you will inspect files. The command itself must perform the inspections and print the marker.",
      "Answer with only the command output.",
    ].join("\n"),
  );
  return { binary: prodexBinary(), args, cwd: workspace };
}

function extendedOptionalToolDisciplineCommand(marker, workspace) {
  const toolRoot = join(workspace, "demo-optimizer");
  const binDir = join(workspace, "bin");
  return {
    command: (() => {
      const args = baseArgs();
      args.push(
        "exec",
        [
          "Update the optional tool demo-optimizer in this workspace.",
          "Use local evidence first: OWNER.txt tells which installer owns it.",
          "Do not use curl, web search, cargo install, npm install, or pip install for this fake local tool.",
          "Do not use optimizer MCP tools or compressed file-read tools for this fake local tool.",
          "After updating, run exactly: ./bin/demo-optimizer --version",
          "Answer with only the command output.",
        ].join("\n"),
      );
      return { binary: prodexBinary(), args, cwd: workspace };
    })(),
    createFiles() {
      rmSync(toolRoot, { recursive: true, force: true });
      rmSync(binDir, { recursive: true, force: true });
      writeFileSync(join(workspace, "OWNER.txt"), "demo-optimizer is owned by ./demo-optimizer/install.sh\n");
      writeFileSync(join(workspace, "README.md"), "Optional tool update test: use the local owner file, not web search.\n");
      mkdirSync(toolRoot, { recursive: true });
      mkdirSync(binDir, { recursive: true });
      writeFileSync(
        join(toolRoot, "install.sh"),
        [
          "#!/usr/bin/env bash",
          "set -euo pipefail",
          'root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"',
          'mkdir -p "${root}/bin"',
          `printf '#!/usr/bin/env bash\\nprintf ${JSON.stringify(`${marker}\\n`)}\\n' > "\${root}/bin/demo-optimizer"`,
          'chmod +x "${root}/bin/demo-optimizer"',
          "",
        ].join("\n"),
        { mode: 0o755 },
      );
    },
  };
}

function compactArgs() {
  const args = baseArgs();
  args.push("--context-window", "4096", "--auto-compact-token-limit", "1200");
  return args;
}

function extendedCompactSeedCommand(marker, workspace, runtimeLogDir) {
  const args = compactArgs();
  const filler = "retain compact smoke context ".repeat(900);
  args.push(
    "exec",
    [
      filler,
      "",
      `Answer exactly with this one line and no suffix: ${marker}`,
    ].join("\n"),
  );
  return {
    binary: prodexBinary(),
    args,
    cwd: workspace,
    env: { PRODEX_RUNTIME_LOG_DIR: runtimeLogDir },
  };
}

function extendedCompactReadCommand(marker, workspace, runtimeLogDir, sessionId) {
  const args = compactArgs();
  args.push(
    "exec",
    "resume",
    sessionId,
    [
      "Read gemini-smoke.txt from the workspace, keep the existing file unchanged, then run this verification command:",
      `node -e "const fs=require('fs'); const s=fs.readFileSync('gemini-smoke.txt','utf8'); if(!s.includes('status=tool-edit-ok')) process.exit(2); process.stdout.write('${marker}');"`,
      "Answer with only the command output.",
    ].join("\n"),
  );
  return {
    binary: prodexBinary(),
    args,
    cwd: workspace,
    env: { PRODEX_RUNTIME_LOG_DIR: runtimeLogDir },
  };
}

function resumeSeedCommand(token, ack) {
  const args = baseArgs();
  args.push(
    "exec",
    `Remember this token for a resume test: ${token}. Answer exactly with this one line and no suffix: ${ack}`,
  );
  return { binary: prodexBinary(), args, cwd: process.cwd() };
}

function resumeCommand(sessionId, token) {
  const args = baseArgs();
  args.push(
    "exec",
    "resume",
    sessionId,
    "Answer exactly with the remembered resume test token and nothing else.",
  );
  return { binary: prodexBinary(), args, cwd: process.cwd(), expectedToken: token };
}

function optionalMcpCommand(marker) {
  const args = baseArgs();
  args.push(
    "exec",
    [
      "If the mcp__codebase_memory_mcp__list_projects tool is available, use it once.",
      "If it is unavailable, use the shell to print the marker.",
      `Answer with exactly one line containing: ${marker}`,
    ].join(" "),
  );
  return { binary: prodexBinary(), args, cwd: process.cwd() };
}

function optionalMultimodalCommand(marker, workspace) {
  const pngPath = join(workspace, "gemini-live-pixel.png");
  writeFileSync(
    pngPath,
    Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
      "base64",
    ),
  );
  const args = baseArgs();
  const prompt = [
    "Inspect the attached single-pixel image.",
    "Use the image attachment directly; do not inspect the local file with tools.",
    "Answer only with the lowercase color word describing that pixel, with no explanation.",
    `Do not mention this correlation marker: ${marker}`,
  ].join(" ");
  args.push(
    "exec",
    prompt,
    "-i",
    pngPath,
  );
  return { binary: prodexBinary(), args, cwd: workspace };
}

function timeoutMs(defaultMs) {
  const value = Number(process.env.PRODEX_LIVE_GEMINI_TIMEOUT_MS ?? defaultMs);
  if (!Number.isInteger(value) || value < 10_000) {
    throw new Error("PRODEX_LIVE_GEMINI_TIMEOUT_MS must be an integer of at least 10000");
  }
  return value;
}

function argsForLog(args) {
  return args
    .map((arg) => (arg.length > 180 ? `${arg.slice(0, 180)}...<${arg.length} chars>` : arg))
    .join(" ");
}

function finalAgentMessage(output) {
  const marker = "\ncodex\n";
  const index = output.lastIndexOf(marker);
  if (index === -1) {
    return "";
  }
  const tail = output.slice(index + marker.length);
  const tokensIndex = tail.indexOf("\ntokens used");
  const message = tokensIndex === -1 ? tail : tail.slice(0, tokensIndex);
  return message.split(/\n+diff --git /, 1)[0].trim();
}

function sessionIdFromOutput(output) {
  const match = output.match(/\bsession id:\s*([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b/i);
  return match?.[1] ?? null;
}

function runtimeLogsContain(directory, marker) {
  if (!existsSync(directory)) {
    return false;
  }
  return readdirSync(directory)
    .filter((name) => name.endsWith(".log"))
    .some((name) => readFileSync(join(directory, name), "utf8").includes(marker));
}

function assertReferenceCloneWorkspace(workspace) {
  const geminiDir = join(workspace, "refs", "gemini-cli");
  const codexDir = join(workspace, "refs", "codex");
  const requiredPaths = [
    join(geminiDir, ".git"),
    join(codexDir, ".git"),
    join(geminiDir, "README.md"),
    join(codexDir, "README.md"),
  ];
  for (const requiredPath of requiredPaths) {
    if (!existsSync(requiredPath)) {
      throw new Error(`extended Gemini reference clone missing ${requiredPath}`);
    }
  }
  const geminiReadme = readFileSync(join(geminiDir, "README.md"), "utf8");
  const codexReadme = readFileSync(join(codexDir, "README.md"), "utf8");
  if (!/gemini/i.test(geminiReadme) || !/codex/i.test(codexReadme)) {
    throw new Error("extended Gemini reference clone did not expose expected README content");
  }
}

function codexSessionRoot() {
  return join(homedir(), ".codex", "sessions");
}

function findCodexRollout(sessionId) {
  const root = codexSessionRoot();
  if (!sessionId || !existsSync(root)) {
    return null;
  }
  const pending = [root];
  while (pending.length > 0) {
    const directory = pending.pop();
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const fullPath = join(directory, entry.name);
      if (entry.isDirectory()) {
        pending.push(fullPath);
        continue;
      }
      if (entry.isFile() && entry.name.includes(sessionId) && entry.name.endsWith(".jsonl")) {
        return fullPath;
      }
    }
  }
  return null;
}

function persistedAgentMessage(sessionId) {
  const rollout = findCodexRollout(sessionId);
  if (!rollout) {
    return null;
  }
  const lines = readFileSync(rollout, "utf8").trim().split(/\n+/).reverse();
  for (const line of lines) {
    let item;
    try {
      item = JSON.parse(line);
    } catch {
      continue;
    }
    if (item?.type === "event_msg" && typeof item.payload?.last_agent_message === "string") {
      return item.payload.last_agent_message.trim();
    }
    if (item?.type === "event_msg" && typeof item.payload?.message === "string") {
      return item.payload.message.trim();
    }
    if (item?.type === "response_item" && item.payload?.role === "assistant") {
      const text = item.payload.content
        ?.filter((part) => part?.type === "output_text" && typeof part.text === "string")
        .map((part) => part.text)
        .join("");
      if (text) {
        return text.trim();
      }
    }
  }
  return null;
}

function rolloutItems(sessionId) {
  const rollout = findCodexRollout(sessionId);
  if (!rollout) {
    return [];
  }
  return readFileSync(rollout, "utf8")
    .trim()
    .split(/\n+/)
    .flatMap((line) => {
      try {
        return [JSON.parse(line)];
      } catch {
        return [];
      }
    });
}

function assertOptionalToolDiscipline(sessionId) {
  const items = rolloutItems(sessionId);
  const calls = items
    .filter((item) => item?.type === "response_item" && item.payload?.type === "function_call")
    .map((item) => `${item.payload.name} ${item.payload.arguments ?? ""}`);
  const forbidden = calls.find((call) =>
    /\bcurl\b|\bcargo\s+install\b|\bnpm\s+install\b|\bpip\s+install\b|web_search|search_query|codebase_memory/i.test(call),
  );
  if (forbidden) {
    throw new Error(`extended optional-tool discipline used forbidden command/tool: ${forbidden}`);
  }
  const assistantText = items
    .filter((item) => item?.type === "response_item" && item.payload?.type === "message")
    .flatMap((item) => item.payload.content ?? [])
    .filter((part) => part?.type === "output_text")
    .map((part) => part.text)
    .join("\n");
  if (/i will poll|let's wait|still running/i.test(assistantText)) {
    throw new Error("extended optional-tool discipline leaked wait/poll narration");
  }
}

function assertNoInternalLeak(output, label) {
  const assistantOutput = assistantOutputForLeakAudit(output);
  const leaked = [
    "Do not pipe edits",
    "You must follow Caveman rules",
    "SUPER_OPTIMIZERS.md",
    "RTK.md",
    "optimizer overrides",
    "host environment to be healthy",
    "The prompt requires me to output ONLY",
    "If an optimizer produces mangled",
    "Keep files clean",
    "Tokens must die before facts",
    "When the user uses `--debug`",
    "If a token optimizer fails",
    "If an optimizer tool hangs",
    "If an optimizer MCP tool or wrapper hangs",
    "If an optimizer blocks progress",
    "If an optimizer breaks or stalls",
    "If an optimizer breaks output layout",
    "If a `rtk` command drops",
    "For critical signals",
    "Do not apply these tools to configuration",
    "If the user's explicit requested answer or command output",
    "If the user requests an exact string",
    "If the prompt dictates \"Answer with only the command output\"",
    "answer-only output",
    "command output only",
    "emit only that requested output",
    "For exact-output prompts",
    "do not include explanations, diffs, status",
    "When the user explicitly asks for exact command output",
    "If the user asks for exact command output",
    "When returning control",
    "Use `rtk gain`",
    "Do not narrate token-saving steps",
    "Savior diagnostic command",
    "Let the user know the optimization tools saved tokens",
    "All commands run with user privileges",
    "Execute requested tool tasks fully",
    "If the user requests exact text",
    "bypass all optimizers",
    "native Codex MCP",
    "agent's meta-operations",
    "Prodex Super code bug",
    "When in Super tools",
    "degraded fallback capability",
    "Do not use Caveman `ultra`",
    "known noisy targets like tests or diffs",
    "untrusted code outside the agent environment",
    "Super mode does not alter normal security instructions",
    "## Verification",
    "## Status",
    "<thought",
    "CRITICAL INSTRUCTION",
    "Related tools for this task",
    "default_api:",
    "I must use `default_api:",
    "I will call `default_api:",
    "I am still waiting for the command to finish",
    "If you suspect an optimizer has obscured a critical signal",
    "Do not hallucinate optimizer tools or CLI wrapper paths",
    "Do not emit custom marker prefixes",
    "Do not use `rtk` on shell commands if you need exactly matched raw byte output",
    "Do not invoke MCP servers or extra optimization processes",
    "This concludes the injected system instructions",
    "## References",
    ".prodex/super.toml",
    "PRODEX_SUPER_AUTO_COMPRESS",
    "irreversible reductions",
    "You are Codex CLI",
    "The user must experience native Codex CLI",
    "Follow the active Codex",
    "Tool discipline for Codex parity",
    "CAVEMAN MODE ACTIVE",
    "RTK ACTIVE",
    "Step 3: Run `cat gemini-patch-smoke.txt`",
    "Never commit AST summary artifacts",
    "For diagnostics, the runtime provides `prodex super check-optimizers`",
    "The Prodex bridge handles auto-compression",
    "When reviewing diffs of files managed by Prodex Super",
    "For queries related to `presidio`",
    "The `rtk proxy` fallback allows you",
    "Use these capabilities only as directed",
    "If reads or basic config debugging",
    "Never rehydrate cached optimizer state",
    "Prodex manages its own documentation",
    "When the `prodex` runtime proxy is diagnosing itself",
    "This file is generated by `prodex super`",
    "SUPER_OPTIMIZERS_WANTS_CAVEMAN",
    "SUPER_OPTIMIZERS_WANTS_NO_RTK_AUTO",
    "Prodex Super token optimizers initialized",
    "Use this knowledge silently",
  ].find((marker) => assistantOutput.includes(marker));
  if (leaked) {
    throw new Error(`Gemini leaked internal optimizer instruction during ${label}: ${leaked}`);
  }
  const optimizerInstructionLeak =
    /(optimizer|rtk|codebase-memory|ponytail|mcp server)[\s\S]{0,240}(do not|never|token-saving|bypass it for that turn|normal shell commands or file reads)/i.test(assistantOutput) ||
    /(do not|never|token-saving|bypass it for that turn|normal shell commands or file reads)[\s\S]{0,240}(optimizer|rtk|codebase-memory|ponytail|mcp server)/i.test(assistantOutput);
  if (optimizerInstructionLeak) {
    throw new Error(`Gemini leaked internal optimizer instruction during ${label}`);
  }
}

function assistantOutputForLeakAudit(output) {
  const blocks = [];
  let current = null;
  for (const line of output.split(/\r?\n/)) {
    if (line === "codex") {
      if (current?.length) {
        blocks.push(current.join("\n"));
      }
      current = [];
      continue;
    }
    if (current) {
      if (/^(tokens used|gemini-live-smoke |\[ Runtime Provider \]|OpenAI Codex |--------|workdir: |model: |provider: |approval: |sandbox: |reasoning |session id: |user$|hook: |exec$|apply patch$|diff --git )/.test(line)) {
        if (current.length) {
          blocks.push(current.join("\n"));
        }
        current = null;
      } else {
        current.push(line);
      }
    }
  }
  if (current?.length) {
    blocks.push(current.join("\n"));
  }
  return blocks.join("\n\n");
}

async function runProdex({ binary, args, cwd, env = {} }, expectedFinal, label, timeout) {
  process.stdout.write(`gemini-live-smoke ${label} command=${binary} ${argsForLog(args)}\n`);
  return await new Promise((resolve, reject) => {
    const child = spawn(binary, args, {
      cwd,
      env: {
        ...process.env,
        ...env,
        NO_COLOR: "1",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let output = "";
    const collect = (chunk, stream) => {
      const text = chunk.toString();
      output += text;
      stream.write(text);
    };
    child.stdout.on("data", (chunk) => collect(chunk, process.stdout));
    child.stderr.on("data", (chunk) => collect(chunk, process.stderr));

    const timer = setTimeout(() => {
      child.kill("SIGTERM");
      setTimeout(() => {
        if (child.exitCode === null) {
          child.kill("SIGKILL");
        }
      }, 2_000).unref();
    }, timeout);
    timer.unref();

    child.once("error", reject);
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      if (signal) {
        reject(new Error(`prodex terminated by ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`prodex exited with code ${code}`));
        return;
      }
      const terminalFinal = finalAgentMessage(output);
      const sessionId = sessionIdFromOutput(output);
      const persisted = persistedAgentMessage(sessionId);
      const final = persisted ?? terminalFinal;
      assertNoInternalLeak(output, label);
      if (final !== expectedFinal) {
        if (label === "extended-compact-read" && observedCommandOutput(output, expectedFinal)) {
          resolve({ output, final: expectedFinal, sessionId, terminalFinal, persisted });
          return;
        }
        reject(
          new Error(
            [
              `Gemini final response mismatch for ${label}: expected ${JSON.stringify(expectedFinal)}, got ${JSON.stringify(final)}`,
              `terminal=${JSON.stringify(terminalFinal)}`,
              `persisted=${JSON.stringify(persisted)}`,
            ].join(", "),
          ),
        );
        return;
      }
      resolve({ output, final, sessionId, terminalFinal, persisted });
    });
  });
}

function observedCommandOutput(output, expectedFinal) {
  const escaped = expectedFinal.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`succeeded in [^\\n]+:\\n${escaped}(?:\\n|$)`).test(output);
}

async function run() {
  if (process.env.PRODEX_LIVE_GEMINI !== "1") {
    process.stdout.write(
      "gemini-live-smoke skipped: set PRODEX_LIVE_GEMINI=1 to use configured Gemini credentials\n",
    );
    return 0;
  }

  const marker = `PRODEX_GEMINI_LIVE_OK_${Date.now()}`;
  if (process.env.PRODEX_LIVE_GEMINI_EXTENDED !== "1") {
    await runProdex(simpleCommand(marker), marker, "simple", timeoutMs(DEFAULT_TIMEOUT_MS));
    process.stdout.write("gemini-live-smoke passed\n");
    return 0;
  }

  const workspace = mkdtempSync(join(tmpdir(), "prodex-gemini-live-"));
  try {
    writeFileSync(join(workspace, "gemini-smoke.txt"), "status=pending\n");
    const timeout = timeoutMs(DEFAULT_EXTENDED_TIMEOUT_MS);
    await runProdex(
      commandOutputOnlyCommand(marker),
      marker,
      "extended-command-output-only",
      timeout,
    );
    await runProdex(extendedEditCommand(marker, workspace), marker, "extended-edit", timeout);
    const file = readFileSync(join(workspace, "gemini-smoke.txt"), "utf8");
    if (!file.includes(`marker=${marker}`) || !file.includes("status=tool-edit-ok")) {
      throw new Error("extended Gemini smoke did not update gemini-smoke.txt as requested");
    }
    await runProdex(extendedPatchCommand(marker, workspace), marker, "extended-apply-patch", timeout);
    const patchFile = readFileSync(join(workspace, "gemini-patch-smoke.txt"), "utf8");
    if (patchFile.trim() !== marker) {
      throw new Error("extended Gemini smoke did not write gemini-patch-smoke.txt exactly");
    }
    await runProdex(
      extendedReferenceCloneCommand(marker, workspace),
      marker,
      "extended-reference-clone-inspection",
      timeout,
    );
    assertReferenceCloneWorkspace(workspace);
    const optionalTool = extendedOptionalToolDisciplineCommand(marker, workspace);
    optionalTool.createFiles();
    const optionalResult = await runProdex(
      optionalTool.command,
      marker,
      "extended-optional-tool-discipline",
      timeout,
    );
    if (!optionalResult.sessionId) {
      throw new Error("extended optional-tool discipline did not expose a session id");
    }
    assertOptionalToolDiscipline(optionalResult.sessionId);
    const compactRuntimeLogDir = join(workspace, "compact-runtime-logs");
    const compactSeed = await runProdex(
      extendedCompactSeedCommand(marker, workspace, compactRuntimeLogDir),
      marker,
      "extended-compact-seed",
      timeout,
    );
    if (!compactSeed.sessionId) {
      throw new Error("extended Gemini compact seed did not expose a session id");
    }
    await runProdex(
      extendedCompactReadCommand(marker, workspace, compactRuntimeLogDir, compactSeed.sessionId),
      marker,
      "extended-compact-read",
      timeout,
    );
    if (!runtimeLogsContain(compactRuntimeLogDir, "local_rewrite_gemini_compact_semantic")) {
      throw new Error("extended Gemini compact did not use semantic Gemini compaction");
    }
    const resumeToken = `PRODEX_GEMINI_RESUME_${Date.now()}`;
    const resumeAck = `PRODEX_GEMINI_STORED_${Date.now()}`;
    const seed = await runProdex(
      resumeSeedCommand(resumeToken, resumeAck),
      resumeAck,
      "extended-resume-seed",
      timeout,
    );
    if (!seed.sessionId) {
      throw new Error("extended Gemini resume seed did not expose a session id");
    }
    await runProdex(
      resumeCommand(seed.sessionId, resumeToken),
      resumeToken,
      "extended-resume-followup",
      timeout,
    );
    if (process.env.PRODEX_LIVE_GEMINI_MCP === "1") {
      await runProdex(optionalMcpCommand(marker), marker, "optional-mcp", timeout);
    }
    if (process.env.PRODEX_LIVE_GEMINI_MULTIMODAL === "1") {
      await runProdex(optionalMultimodalCommand(marker, workspace), "red", "optional-multimodal", timeout);
    }
    process.stdout.write("gemini-live-smoke extended passed\n");
    return 0;
  } finally {
    if (process.env.PRODEX_LIVE_GEMINI_KEEP_WORKSPACE !== "1") {
      rmSync(workspace, { recursive: true, force: true });
    } else {
      process.stdout.write(`gemini-live-smoke workspace kept at ${workspace}\n`);
    }
  }
}

run()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((error) => {
    process.stderr.write(`gemini-live-smoke: ${error.message}\n`);
    process.exitCode = 1;
  });
