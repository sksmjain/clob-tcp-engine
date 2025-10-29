const { parseArgs } = require("./helpers.js");

const host = parseArgs.host || "127.0.0.1";   // server host
const port = parseInt(parseArgs.port || "9000", 10); // server port
const clients = parseInt(parseArgs.clients || "5", 10); // number of clients
const intervalMs = parseInt(parseArgs.interval || "5000", 10); // per-client period
const durationSec = parseArgs.duration ? parseInt(parseArgs.duration, 10) : null; // optional test duration
const typeDefault = parseInt(parseArgs.type || "1", 10); // msg type (1 = PING)
const localPortStart = parseArgs["local-port-start"] ? parseInt(parseArgs["local-port-start"], 10) : null; // optional fixed local ports
const quiet = !!parseArgs.quiet;

function usageAndExit() {
  console.log(`
    Usage:
      node client.js ping

      node client.js new --client <u64> --id <u64> --side <buy|sell|0|1> --price <i64> --qty <i64> --tif <gtc|ioc|0|1>

      node client.js cancel --client <u64> --id <u64>

    Env/config:
      HOST, PORT via ./config.js
    `);
      process.exit(1);
}

module.exports = {
  host,
  port,
  clients,
  intervalMs,
  durationSec,
  typeDefault,
  localPortStart,
  quiet,
  usageAndExit
};