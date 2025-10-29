#!/usr/bin/env node
const net = require("node:net");
const readline = require("node:readline");
const { argv } = require("node:process");

// ---------- CLI args ----------
const args = Object.fromEntries(
  argv.slice(2).map(a => {
    const [k, v] = a.replace(/^--/, "").split("=");
    return [k, v ?? true];
  })
);

const host = args.host || "127.0.0.1";
const port = parseInt(args.port || "9000", 10);
const localPort = args.localport ? parseInt(args.localport, 10) : undefined;

// ---------- framing helpers ----------
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n, 0); return b; };
const u16 = n => { const b = Buffer.alloc(2); b.writeUInt16LE(n, 0); return b; };

function buildFrame(type, bodyBuf) {
  const payload = Buffer.concat([u16(type), u16(bodyBuf.length), bodyBuf]);
  return Buffer.concat([u32(payload.length), payload]);
}

// ---------- connect ----------
const socket = net.createConnection({ host, port, localPort }, () => {
  const local = socket.address();
  console.log(`✅ Connected (local ${local.address}:${local.port}) → ${host}:${port}`);
  console.log(`Type messages and press Enter to send. Use ":q" to quit.`);
  rl.prompt();
});

socket.on("error", e => console.error("Socket error:", e.message));
socket.on("end",   () => console.log("Disconnected by server"));
socket.on("close", () => process.exit(0));

// ---------- handle server responses ----------
let rx = Buffer.alloc(0);
socket.on("data", chunk => {
  rx = Buffer.concat([rx, chunk]);
  while (rx.length >= 4) {
    const len = rx.readUInt32LE(0);
    if (rx.length < 4 + len) break;

    const p = rx.subarray(4, 4 + len);
    const t = p.readUInt16LE(0);
    const bl = p.readUInt16LE(2);
    const body = p.subarray(4, 4 + bl).toString();

    console.log(`\n← [type=${t}] "${body}"`);
    rx = rx.subarray(4 + len);
    rl.prompt();
  }
});

// ---------- REPL ----------
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  prompt: "-> "
});

rl.on("line", line => {
  const s = line.trim();
  if (s === ":q" || s === ":quit") {
    rl.close();
    socket.end();
    return;
  }

  const msgType = 1; // ping
  const frame = buildFrame(msgType, Buffer.from(s));
  socket.write(frame);
  rl.prompt();
});

rl.on("SIGINT", () => {
  console.log("\n^C");
  rl.close();
  socket.end();
});
