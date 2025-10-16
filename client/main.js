#!/usr/bin/env node
// tcp-cli.js

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

if (args.help || args.h) {
  console.log(`Usage: ./tcp-cli.js --host=localhost --port=9000 [--type=1]
Type a line and press Enter to send. Use "t:2 your text" to override type per message. Type :q to quit.`);
  process.exit(0);
}

const host = args.host || "localhost";
const port = parseInt(args.port || "9000", 10);
const defaultType = parseInt(args.type || "1", 10);

// ---------- helpers ----------
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n, 0); return b; };
const u16 = n => { const b = Buffer.alloc(2); b.writeUInt16LE(n, 0); return b; };

function buildFrame(type, bodyBuf) {
  const payload = Buffer.concat([u16(type), u16(bodyBuf.length), bodyBuf]);
  return Buffer.concat([u32(payload.length), payload]);
}

// ---------- stdin REPL ----------
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  prompt: "-> "
});

// ---------- connect ----------
const socket = net.createConnection({ host, port }, () => {
  console.log(`Connected to ${host}:${port}`);
  console.log(`Type messages and press Enter to send. Use ":q" to quit.`);
  console.log(`Tip: override type per-message like "t:2 hello world"`);
  rl.prompt();
});

socket.on("error", (e) => console.error("Socket error:", e.message));
socket.on("end",   () => console.log("Disconnected by server"));
socket.on("close", () => process.exit(0));

// ---------- incoming data (framed) ----------
let rx = Buffer.alloc(0);
socket.on("data", (chunk) => {
  rx = Buffer.concat([rx, chunk]);
  while (rx.length >= 4) {
    const len = rx.readUInt32LE(0);
    if (rx.length < 4 + len) break;

    const p = rx.subarray(4, 4 + len);
    const t = p.readUInt16LE(0);
    const bl = p.readUInt16LE(2);
    const body = p.subarray(4, 4 + bl).toString();
    console.log(`\n<- type=${t} body="${body}"`);

    rx = rx.subarray(4 + len);
    rl.prompt();
  }
});

// ---------- send lines ----------
rl.on("line", (line) => {
  const s = line.trim();
  if (s === ":q" || s === ":quit" || s === ":exit") {
    rl.close();
    socket.end();
    return;
  }

  // Per-message type override: "t:2 your text"
  let msgType = defaultType;
  let text = s;
  const m = s.match(/^t:(\d+)\s+(.*)$/);
  if (m) {
    msgType = parseInt(m[1], 10);
    text = m[2];
  }

  const frame = buildFrame(msgType, Buffer.from(text));
  socket.write(frame);
  rl.prompt();
});

rl.on("SIGINT", () => {
  console.log("\n^C");
  rl.close();
  socket.end();
});
