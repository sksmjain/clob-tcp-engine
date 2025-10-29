const { parse } = require("node:path");
const { argv } = require("node:process");

// ---- helpers (LE encoders) ----
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n,0); return b; };
const u16 = n => { const b = Buffer.alloc(2); b.writeUInt16LE(n,0); return b; };
const u64 = n => { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(n),0); return b; };
const i64 = n => { const b = Buffer.alloc(8); b.writeBigInt64LE(BigInt(n),0); return b; };

function buildFrame(type, bodyBuf = Buffer.alloc(0)) {
    const payload = Buffer.concat([u16(type), u16(bodyBuf.length), bodyBuf]);
    return Buffer.concat([u32(payload.length), payload]);
}

// ---------- CLI args parser ----------
function parseArgs(argv) {
  // Examples:
  //   node client.js ping
  //   node client.js new --client 2 --id 2001 --side buy --price 101000 --qty 5000 --tif gtc
  //   node client.js cancel --client 2 --id 2001

  const out = { cmd: null, flags: {} };
  const args = argv.slice(2);
  if (args.length === 0) return out;

  out.cmd = args[0]; // ping | new | cancel

  // parse --k v pairs
  for (let i = 1; i < args.length; i++) {
    const k = args[i];
    if (!k.startsWith("--")) continue;
    const key = k.slice(2);
    const v = args[i + 1] && !args[i + 1].startsWith("--") ? args[++i] : "true";
    out.flags[key] = v;
  }
  return out;
}


// 
module.exports = {
  u32,
  u16,
  u64,
  i64,
  buildFrame,
  parseArgs,
};