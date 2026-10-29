import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import https from "node:https";
import net from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// The local CONNECT proxy intercepts the production origin without DNS or provider traffic.
const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const constants = readFileSync(join(root, "crates/prodex-app/src/core_constants.rs"), "utf8");
const baseUrl = constants.match(/DEFAULT_CHATGPT_BASE_URL: &str = "([^"]+)"/)?.[1];
assert(baseUrl, "default upstream URL");
const host = new URL(baseUrl).hostname;
const temp = mkdtempSync(join(tmpdir(), "prodex-guardian-tls-"));

function frame(value) {
  const payload = Buffer.from(JSON.stringify(value));
  const header = payload.length < 126
    ? Buffer.from([0x81, payload.length])
    : Buffer.from([0x81, 126, payload.length >> 8, payload.length & 255]);
  return Buffer.concat([header, payload]);
}

try {
  const openssl = (args) => execFileSync("openssl", args, { cwd: temp, stdio: "ignore" });
  openssl(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "ca.key", "-out", "ca.pem", "-days", "1", "-subj", "/CN=Local Test Root"]);
  openssl(["req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", "server.key", "-out", "server.csr", "-subj", `/CN=${host}`, "-addext", `subjectAltName=DNS:${host}`]);
  openssl(["x509", "-req", "-in", "server.csr", "-CA", "ca.pem", "-CAkey", "ca.key", "-CAcreateserial", "-out", "server.pem", "-days", "1", "-copy_extensions", "copy"]);

  const sockets = new Set();
  const handshakes = [];
  const server = https.createServer({
    key: readFileSync(join(temp, "server.key")),
    cert: readFileSync(join(temp, "server.pem")),
  }, (req, res) => {
    if (!req.url?.endsWith("/wham/usage")) {
      res.writeHead(404).end();
      return;
    }
    const now = Math.floor(Date.now() / 1000);
    res.writeHead(200, { "content-type": "application/json" }).end(JSON.stringify({
      email: "test@example.com",
      plan_type: "plus",
      rate_limit: {
        primary_window: { used_percent: 5, reset_at: now + 18000, limit_window_seconds: 18000 },
        secondary_window: { used_percent: 5, reset_at: now + 604800, limit_window_seconds: 604800 },
      },
    }));
  });
  server.on("upgrade", (req, socket, head) => {
    const sequence = handshakes.length + 1;
    handshakes.push({ cookie: req.headers.cookie ?? "", guardian: req.headers["x-codex-guardian"] ?? "" });
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
    const accept = createHash("sha1")
      .update(`${req.headers["sec-websocket-key"]}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`)
      .digest("base64");
    const cookie = sequence === 1 ? "parent" : sequence === 2 ? "later" : "reviewer";
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\nSet-Cookie: __oailb=${cookie}; Path=/; Secure; Max-Age=300\r\n\r\n`);
    let buffered = head;
    socket.on("data", (chunk) => {
      buffered = Buffer.concat([buffered, chunk]);
      while (buffered.length >= 2) {
        const opcode = buffered[0] & 15;
        let length = buffered[1] & 127;
        let offset = 2;
        if (length === 126) {
          if (buffered.length < 4) return;
          length = buffered.readUInt16BE(2);
          offset = 4;
        } else if (length === 127) {
          if (buffered.length < 10) return;
          length = Number(buffered.readBigUInt64BE(2));
          offset = 10;
        }
        const maskLength = buffered[1] & 128 ? 4 : 0;
        if (buffered.length < offset + maskLength + length) return;
        buffered = buffered.subarray(offset + maskLength + length);
        if (opcode === 8) {
          socket.end();
          return;
        }
        if (opcode !== 1) continue;
        const id = sequence === 1 ? "resp-parent-tls" : `resp-${sequence}`;
        socket.write(frame({ type: "response.created", response: { id } }));
        socket.write(frame({ type: "response.output_text.delta", response: { id }, delta: "synthetic" }));
        socket.write(frame({ type: "response.completed", response: { id, status: "completed", output: [] } }));
      }
    });
  });
  await new Promise((resolve, reject) => server.once("error", reject).listen(0, "127.0.0.1", resolve));
  const serverPort = server.address().port;
  const proxy = net.createServer((socket) => {
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
    let input = Buffer.alloc(0);
    function readConnect(chunk) {
      input = Buffer.concat([input, chunk]);
      const end = input.indexOf("\r\n\r\n");
      if (end < 0) return;
      socket.off("data", readConnect);
      if (!input.subarray(0, end).toString().startsWith(`CONNECT ${host}:443 HTTP/1.1`)) {
        socket.end("HTTP/1.1 403 Forbidden\r\n\r\n");
        return;
      }
      const upstream = net.connect(serverPort, "127.0.0.1", () => {
        socket.write("HTTP/1.1 200 Connection Established\r\n\r\n");
        const rest = input.subarray(end + 4);
        if (rest.length) upstream.write(rest);
        socket.pipe(upstream).pipe(socket);
      });
      upstream.on("error", () => socket.destroy());
      socket.on("error", () => upstream.destroy());
    }
    socket.on("data", readConnect);
  });
  await new Promise((resolve, reject) => proxy.once("error", reject).listen(0, "127.0.0.1", resolve));
  const proxyUrl = `http://127.0.0.1:${proxy.address().port}`;
  const env = {
    ...process.env,
    HTTPS_PROXY: proxyUrl, https_proxy: proxyUrl, HTTP_PROXY: proxyUrl, http_proxy: proxyUrl,
    ALL_PROXY: proxyUrl, all_proxy: proxyUrl, NO_PROXY: "", no_proxy: "",
    SSL_CERT_FILE: join(temp, "ca.pem"), CARGO_NET_OFFLINE: "true",
    PRODEX_MOJO_REQUIRED: "1", PRODEX_MOJO_VERSION: "1.2.0",
  };
  const child = spawn("cargo", ["test", "--locked", "-q", "-p", "prodex-app", "--lib",
    "runtime_proxy_guardian_tls_parent_pin_end_to_end", "--", "--ignored", "--test-threads=1"],
  { cwd: root, env, stdio: "inherit" });
  const timeout = setTimeout(() => child.kill("SIGKILL"), 180000);
  let result;
  try {
    result = await new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("exit", (code) => resolve(code));
    });
  } finally {
    clearTimeout(timeout);
    for (const socket of sockets) socket.destroy();
    await Promise.all([
      new Promise((resolve) => proxy.close(resolve)),
      new Promise((resolve) => server.close(resolve)),
    ]);
  }
  assert.equal(result, 0, "isolated Rust test");
  assert.equal(handshakes.length, 3);
  assert.match(handshakes[1].cookie, /__oailb=parent/);
  assert.equal(handshakes[2].guardian, "reviewer");
  assert.match(handshakes[2].cookie, /__oailb=parent/);
  assert.doesNotMatch(handshakes[2].cookie, /__oailb=later/);
  console.log("isolated TLS guardian routing smoke: passed");
} finally {
  rmSync(temp, { recursive: true, force: true });
}
