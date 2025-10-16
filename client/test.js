#!/usr/bin/env node
/**
 * test.js — spawn N TCP clients, each sending 1 msg/sec to a single server.
 * Tracks total sent, total acks, backlog, and latest processing info.
 */

const net = require("node:net");
const { argv } = require("node:process");

// ---------------- CLI ----------------
const args = Object.fromEntries(
  argv.slice(2).map(a => {
    const [k, v] = a.replace(/^--/, "").split("=");
    return [k, v ?? true];
  })
);

const host = args.host || "127.0.0.1";   // server host
const port = parseInt(args.port || "9000", 10); // server port
const clients = parseInt(args.clients || "5", 10); // number of clients
const intervalMs = parseInt(args.interval || "5000", 10); // per-client period
const durationSec = args.duration ? parseInt(args.duration, 10) : null; // optional test duration
const typeDefault = parseInt(args.type || "1", 10); // msg type (1 = PING)
const localPortStart = args["local-port-start"] ? parseInt(args["local-port-start"], 10) : null; // optional fixed local ports
const quiet = !!args.quiet;

// ------------- framing helpers -------------
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n, 0); return b; };
const u16 = n => { const b = Buffer.alloc(2); b.writeUInt16LE(n, 0); return b; };

function buildFrame(type, bodyBuf) {
  const payload = Buffer.concat([u16(type), u16(bodyBuf.length), bodyBuf]);
  return Buffer.concat([u32(payload.length), payload]);
}

// simple framed reader that resolves frames in order
function makeFramedReader(socket, onFrame) {
  let rx = Buffer.alloc(0);
  socket.on("data", chunk => {
    rx = Buffer.concat([rx, chunk]);
    while (rx.length >= 4) {
      const len = rx.readUInt32LE(0);
      if (rx.length < 4 + len) break;

      const frame = rx.subarray(4, 4 + len);
      rx = rx.subarray(4 + len);

      const t = frame.readUInt16LE(0);
      const bl = frame.readUInt16LE(2);
      const body = frame.subarray(4, 4 + bl);
      onFrame({ type: t, body });
    }
  });
}

// ---------------- metrics ----------------
const stats = {
  startedAt: Date.now(),
  sentTotal: 0,
  ackTotal: 0,
  clients: Array.from({ length: clients }, () => ({ sent: 0, ack: 0, lastSeq: 0 })),
  latestAck: null, // { clientId, seq, micros, body }
};

function printReport() {
  const backlog = stats.sentTotal - stats.ackTotal;
  const elapsedSec = ((Date.now() - stats.startedAt) / 1000).toFixed(1);
  const latest = stats.latestAck
    ? `latest ack: c${stats.latestAck.clientId} seq=${stats.latestAck.seq} ${stats.latestAck.micros}µs body="${stats.latestAck.body}"`
    : "latest ack: (none yet)";
  console.log(
    `[t=${elapsedSec}s] sent=${stats.sentTotal} ack=${stats.ackTotal} backlog=${backlog} | ${latest}`
  );
}

// ---------------- client logic ----------------
function hrNow() { return process.hrtime.bigint(); } // ns
function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

async function runClient(idx) {
  return new Promise((resolve, reject) => {
    const localPort = localPortStart != null ? localPortStart + idx : undefined;
    const socket = net.createConnection({ host, port, localPort }, async () => {
      if (!quiet) {
        const a = socket.address();
        console.log(`[c${idx}] connected (local ${a.address}:${a.port}) -> ${host}:${port}`);
      }

      const pending = new Map(); // seq -> t0 (bigint)
      makeFramedReader(socket, ({ type, body }) => {
        // An ACK per message is assumed
        const ackStr = body.toString();
        // Extract seq if server echoes it? Our server replies "pong", so we just match on first pending seq
        // Pull the oldest pending seq:
        const it = pending.keys();
        const { value: seq, done } = it.next();
        if (!done) {
          const t0 = pending.get(seq);
          pending.delete(seq);
          const micros = Number((hrNow() - t0) / 1000n);

          stats.ackTotal++;
          stats.clients[idx].ack++;
          stats.latestAck = { clientId: idx, seq, micros, body: ackStr };
          if (!quiet) console.log(`[c${idx}] <- ack (seq=${seq}) in ${micros}µs body="${ackStr}"`);
        } else {
          if (!quiet) console.log(`[c${idx}] <- unexpected ack (no pending seq) body="${ackStr}"`);
        }
      });

      // send loop
      let seq = 0;
      const timer = setInterval(() => {
        try {
          seq++;
          const payload = Buffer.from(`client ${idx} seq ${seq}`);
          const frame = buildFrame(typeDefault, payload);
          pending.set(seq, hrNow());
          socket.write(frame);
          stats.sentTotal++;
          stats.clients[idx].sent++;
          stats.clients[idx].lastSeq = seq;
          if (!quiet) console.log(`[c${idx}] -> msg (seq=${seq})`);
        } catch (e) {
          console.error(`[c${idx}] send error:`, e.message);
        }
      }, intervalMs);

      // optional duration
      if (durationSec != null) {
        setTimeout(() => {
          clearInterval(timer);
          socket.end();
          resolve();
        }, durationSec * 1000);
      }

      socket.on("error", err => {
        console.error(`[c${idx}] socket error:`, err.message);
        clearInterval(timer);
        reject(err);
      });

      socket.on("end", () => {
        if (!quiet) console.log(`[c${idx}] disconnected by server`);
        clearInterval(timer);
        resolve();
      });

      socket.on("close", () => {
        clearInterval(timer);
      });
    });
  });
}

// --------------- main orchestration ---------------
(async () => {
  // periodic reporting
  const reportTimer = setInterval(printReport, 1000);

  const jobs = [];
  for (let i = 0; i < clients; i++) jobs.push(runClient(i));

  try {
    await Promise.all(jobs);
    clearInterval(reportTimer);
    printReport();
    console.log(`✅ all ${clients} clients finished`);
    process.exit(0);
  } catch (e) {
    clearInterval(reportTimer);
    console.error(`💥 test run error:`, e);
    process.exit(1);
  }
})();
