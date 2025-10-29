// main.js
const net = require("node:net");
const fs = require("node:fs");
const readline = require("node:readline");
const { host, port, localPort } = require("./config");
const { buildFrame, u64, i64 } = require("./helpers");

// ---------------- CLI args ----------------
const args = Object.fromEntries(
  process.argv.slice(2).map(a => {
    const [k, v] = a.replace(/^--/, "").split("=");
    return [k, v ?? true];
  })
);
const TEST_MODE = !!args.test;
const TEST_FILE =
  (typeof args.test === "string" && args.test) ||
  args.file ||
  "orders.txt";
const SLEEP_MS = args.sleep ? Number(args.sleep) : 0;
const LOOP = !!args.loop;

// ---------- encoders & frames ----------
function ping() { return buildFrame(1); }
function newOrder({ client_id, cl_ord_id, side, price, qty, tif }) {
  const payload = Buffer.concat([
    u64(client_id),
    u64(cl_ord_id),
    Buffer.from([side]),          // 0=bid, 1=ask
    i64(price),
    i64(qty),
    Buffer.from([tif]),           // 0=GTC, 1=IOC
  ]);
  return buildFrame(10, payload);
}
function cancel({ client_id, cl_ord_id }) {
  const payload = Buffer.concat([u64(client_id), u64(cl_ord_id)]);
  return buildFrame(11, payload);
}

// ---------- small parsers ----------
function toSide(v) {
  if (v === undefined) throw new Error("side is required");
  const s = String(v).toLowerCase();
  if (s === "0" || s === "bid"  || s === "buy")  return 0;
  if (s === "1" || s === "ask"  || s === "sell") return 1;
  throw new Error("side must be bid|ask|0|1");
}
function toTif(v) {
  if (v === undefined) throw new Error("tif is required");
  const t = String(v).toLowerCase();
  if (t === "0" || t === "gtc") return 0;
  if (t === "1" || t === "ioc") return 1;
  throw new Error("tif must be gtc|ioc|0|1");
}
function parseKV(tokens) {
  const out = {};
  for (const tok of tokens) {
    const [k, v] = tok.split("=");
    if (!k) continue;
    out[k.toLowerCase()] = v;
  }
  return out;
}
const HELP = `
Commands:
  ping
  new client=<u64> id=<u64> side=<bid|ask|0|1> price=<i64> qty=<i64> tif=<gtc|ioc|0|1>
  cancel client=<u64> id=<u64>
  help
  quit | :q | exit

Examples:
  ping
  new client=2 id=2001 side=ask price=101000 qty=5000 tif=gtc
  new client=3 id=3001 side=bid price=101000 qty=2000 tif=ioc
  cancel client=2 id=2001
`;

// ---------- TCP connect ----------
const connectOpts = { host, port };
if (typeof localPort === "number") connectOpts.localPort = localPort;

console.log("\n===============================");
console.log("\x1b[36m🚀 CLOB TCP Client\x1b[0m", TEST_MODE ? "(test mode)" : "(interactive)");
console.log("===============================");
console.log(`→ Target: ${host}:${port}`);
if (TEST_MODE) {
  console.log(`→ File: ${TEST_FILE}  |  Sleep: ${SLEEP_MS}ms  |  Loop: ${LOOP ? "yes" : "no"}`);
}
console.log("===============================\n");

const socket = net.createConnection(connectOpts, () => {
  const local = socket.address();
  console.log(`✅ Connected (local ${local.address}:${local.port}) → ${host}:${port}`);
  socket.setNoDelay(true);
  socket.setKeepAlive(true, 10_000);

  if (TEST_MODE) {
    runFile(TEST_FILE, { sleepMs: SLEEP_MS, loop: LOOP });
  } else {
    console.log(HELP.trim() + "\n");
    rl.prompt();
  }
});

// ---------- shared command handler ----------
function handleCommand(text) {
  const line = String(text || "").trim();
  if (!line || line.startsWith("#")) return; // allow comments in files

  if (line === "quit" || line === ":q" || line === "exit") {
    gracefulClose();
    return;
  }
  if (line === "help" || line === "?") {
    console.log(HELP.trim());
    if (!TEST_MODE) rl.prompt();
    return;
  }

  const [cmdRaw, ...rest] = line.split(/\s+/);
  const cmd = (cmdRaw || "").toLowerCase();

  try {
    if (cmd === "ping") {
      console.log("📤 \x1b[34mPING\x1b[0m");
      socket.write(ping());
      if (!TEST_MODE) rl.prompt();
      return;
    }

    if (cmd === "new" || cmd === "order" || cmd === "neworder") {
      const kv = parseKV(rest);
      const client_id = BigInt(kv.client ?? kv.client_id ?? (()=>{throw new Error("missing client id");})());
      const cl_ord_id = BigInt(kv.id ?? kv.cl_ord_id ?? (()=>{throw new Error("missing order id");})());
      const side = toSide(kv.side);
      const tif  = toTif(kv.tif);
      const price = BigInt(kv.price ?? (()=>{throw new Error("missing price");})());
      const qty   = BigInt(kv.qty   ?? (()=>{throw new Error("missing qty");})());

      console.log("📤 \x1b[34mNEW_ORDER\x1b[0m", {
        client_id: client_id.toString(),
        cl_ord_id: cl_ord_id.toString(),
        side: side === 0 ? "BID" : "ASK",
        price: price.toString(),
        qty: qty.toString(),
        tif: tif === 0 ? "GTC" : "IOC",
      });
      socket.write(newOrder({ client_id, cl_ord_id, side, price, qty, tif }));
      if (!TEST_MODE) rl.prompt();
      return;
    }

    if (cmd === "cancel") {
      const kv = parseKV(rest);
      const client_id = BigInt(kv.client ?? kv.client_id ?? (()=>{throw new Error("missing client id");})());
      const cl_ord_id = BigInt(kv.id ?? kv.cl_ord_id ?? (()=>{throw new Error("missing order id");})());
      console.log("📤 \x1b[34mCANCEL\x1b[0m", {
        client_id: client_id.toString(),
        cl_ord_id: cl_ord_id.toString(),
      });
      socket.write(cancel({ client_id, cl_ord_id }));
      if (!TEST_MODE) rl.prompt();
      return;
    }

    console.log(`\x1b[31mUnknown command:\x1b[0m ${cmd}`);
    if (!TEST_MODE) {
      console.log(HELP.trim());
      rl.prompt();
    }
  } catch (e) {
    console.error("\x1b[31mInput error:\x1b[0m", e.message);
    if (!TEST_MODE) rl.prompt();
  }
}

// ---------- interactive REPL (only created for interactive mode) ----------
let rl = null;
if (!TEST_MODE) {
  rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt: "\x1b[33mclob>\x1b[0m ",
  });

  rl.on("line", handleCommand);
  rl.on("SIGINT", gracefulClose); // Ctrl+C
}

// ---------- test runner ----------
async function runFile(path, { sleepMs = 0, loop = false } = {}) {
  if (!fs.existsSync(path)) {
    console.error(`\x1b[31mTest file not found:\x1b[0m ${path}`);
    gracefulClose(1);
    return;
  }
  console.log(`▶️  Running test file: ${path}`);

  const runOnce = () =>
    new Promise((resolve, reject) => {
      const rls = readline.createInterface({
        input: fs.createReadStream(path),
        crlfDelay: Infinity,
      });

      rls.on("line", async (line) => {
        handleCommand(line);
        if (sleepMs > 0) await new Promise(r => setTimeout(r, sleepMs));
      });
      rls.once("close", resolve);
      rls.once("error", reject);
    });

  do {
    await runOnce();
    if (loop) console.log("↩️  Looping test file...");
  } while (loop);

  console.log("✅ Test file completed.");
  gracefulClose(0);
}

// ---------- receive & decode frames ----------
let buf = Buffer.alloc(0);
socket.on("data", (chunk) => {
  buf = Buffer.concat([buf, chunk]);

  while (buf.length >= 4) {
    const len = buf.readUInt32LE(0);
    if (buf.length < 4 + len) break;

    const body = buf.subarray(4, 4 + len);
    const type = body.readUInt16LE(0);

    console.log("\n🧾 \x1b[35mReceived Frame\x1b[0m — Type:", type);

    if (type === 100) { // ACK (and PONG as ACK+"pong")
      const cl = body.readBigUInt64LE(2);
      const l  = body.readUInt16LE(10);
      const txt = body.subarray(12, 12 + l).toString();
      console.log("✅ \x1b[32mACK\x1b[0m", { cl: cl.toString(), text: txt });
    } else if (type === 199) { // REJECT
      const cl = body.readBigUInt64LE(2);
      const l  = body.readUInt16LE(10);
      const reason = body.subarray(12, 12 + l).toString();
      console.log("❌ \x1b[31mREJECT\x1b[0m", { cl: cl.toString(), reason });
    } else if (type === 101) { // TRADE
      const price = body.readBigInt64LE(2);
      const qty   = body.readBigInt64LE(10);
      const tak   = body.readBigUInt64LE(18);
      const mak   = body.readBigUInt64LE(26);
      console.log("💥 \x1b[33mTRADE\x1b[0m", {
        price: price.toString(),
        qty: qty.toString(),
        tak: tak.toString(),
        mak: mak.toString(),
      });
    } else if (type === 102) { // BOOK_DELTA
      const side  = body.readUInt8(2) === 0 ? "BID" : "ASK";
      const price = body.readBigInt64LE(3);
      const lvl   = body.readBigInt64LE(11);
      console.log("📊 \x1b[36mBOOK_DELTA\x1b[0m", {
        side,
        price: price.toString(),
        level: lvl.toString(),
      });
    } else {
      console.log("❓ \x1b[90mUNKNOWN EVT\x1b[0m", { type });
    }

    buf = buf.subarray(4 + len);
  }
  if (!TEST_MODE && rl) rl.prompt();
});

// ---------- lifecycle ----------
socket.on("error", (e) => {
  console.error("\x1b[31mSocket error:\x1b[0m", e.message);
  gracefulClose(1);
});
socket.on("end",   () => console.log("\x1b[33mDisconnected by server\x1b[0m"));
socket.on("close", () => {
  console.log("\n👋 \x1b[90mConnection closed\x1b[0m");
  process.exit(0);
});

function gracefulClose(code = 0) {
  try { socket.end(); } catch {}
  try { if (rl) rl.close(); } catch {}
  process.exit(code);
}
