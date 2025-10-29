#!/usr/bin/env node
const net = require("node:net");
const readline = require("node:readline");
const { argv } = require("node:process");
const { buildFrame } = require("../helpers");
const { host, port, localPort } = require("../config");


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
