use tokio::{
    io::{BufReader, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use bytes::{BytesMut, Buf, BufMut};
use std::convert::TryInto;

const MSG_PING: u16 = 1;
const MSG_ACK:  u16 = 100;

async fn process(mut socket: TcpStream) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(16 * 1024);

    let addr = socket.peer_addr()?;
    println!("\n📡 Started session with client {addr}");

    loop {
        // =============== READ ===============
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 {
            println!("❌ Client {addr} disconnected");
            break;
        }
        println!("🟦 Received {n} bytes from {addr} (total buffered: {})", buf.len());

        // =============== DECODE ===============
        loop {
            if buf.len() < 4 { break; }

            let payload_len = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
            if buf.len() < 4 + payload_len { break; }

            let mut frame = buf.split_to(4 + payload_len);
            frame.advance(4); // drop length

            if frame.len() < 4 {
                println!("⚠️  Malformed frame (too short for headers)");
                continue;
            }

            let msg_type = frame.get_u16_le();
            let body_len = frame.get_u16_le() as usize;

            if frame.len() < body_len {
                println!("⚠️  Malformed frame (body_len={} but only {} left)", body_len, frame.len());
                continue;
            }

            let body = frame.split_to(body_len);
            let body_str = String::from_utf8_lossy(&body);

            println!(
                "📩 Decoded frame → type: {msg_type}, body_len: {body_len}, body: \"{}\"",
                body_str
            );

            // =============== HANDLE ===============
            match msg_type {
                MSG_PING => {
                    println!("🔁 PING received → sending ACK: pong");
                    ack(&mut socket, b"pong").await?;
                }
                _ => {
                    println!("❔ Unknown message type {msg_type} → sending empty ACK");
                    ack(&mut socket, b"").await?;
                }
            }
        }
    }

    Ok(())
}

/// send: [u32 len][u16 MSG_ACK][u16 body_len][body…]
async fn ack(sock: &mut TcpStream, body: &[u8]) -> anyhow::Result<()> {
    let total = 2 + 2 + body.len();
    let mut out = BytesMut::with_capacity(4 + total);
    out.put_u32_le(total as u32);
    out.put_u16_le(MSG_ACK);
    out.put_u16_le(body.len() as u16);
    out.extend_from_slice(body);
    sock.write_all(&out).await?;

    println!("✅ Sent ACK with body: \"{}\"", String::from_utf8_lossy(body));
    println!("🏁 Message processed and acknowledged\n");
    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9000").await?;
    println!("🚀 Server listening on {}", listener.local_addr()?);

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("✅ Accepted new client: {addr}");

        tokio::spawn(async move {
            if let Err(e) = process(socket).await {
                eprintln!("💥 Error processing {addr}: {e}");
            }
        });
    }
}
