const net = require('node:net');

const u32 = n => {const b=Buffer.alloc(4); b.writeUInt32LE(n,0); return b};
const u16 = n => {const b=Buffer.alloc(2); b.writeUInt16LE(n,0); return b};

const ping = Buffer.concat([u16(1)]);
const frame = Buffer.concat([u32(ping.length), ping]);

const socket = net.createConnection({ port: 9000, host: 'localhost' }, () => {
    console.log('connected to server');
    socket.write(frame);
});

let buf = Buffer.alloc(0);
socket.on("data", (chunk)=>{
  buf = Buffer.concat([buf, chunk]);
  while (buf.length >= 4) {
    const len = buf.readUInt32LE(0);
    if (buf.length < 4+len) break;
    const p = buf.subarray(4, 4+len);
    const t = p.readUInt16LE(0);
    const bl = p.readUInt16LE(2);
    const body = p.subarray(4, 4+bl).toString();
    console.log({ type: t, body });
    buf = buf.subarray(4+len);
  }
});