use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::{interval, Duration},
};
use bytes::{BytesMut, Buf, BufMut};
use std::{convert::TryInto, time::Instant};

const MSG_PING: u16 = 1;
const MSG_ACK:  u16 = 100;

/// Send: [u32 len][u16 MSG_ACK][u16 body_len][body…]
async fn ack(sock: &mut TcpStream, body: &[u8]) -> anyhow::Result<()> {
    let total = 2 + 2 + body.len();
    let mut out = BytesMut::with_capacity(4 + total);
    out.put_u32_le(total as u32);
    out.put_u16_le(MSG_ACK);
    out.put_u16_le(body.len() as u16);
    out.extend_from_slice(body);
    sock.write_all(&out).await?;
    Ok(())
}

async fn process(mut socket: TcpStream, lat_tx: mpsc::UnboundedSender<u64>) -> anyhow::Result<()> {
    socket.set_nodelay(true)?;
    let mut buf = BytesMut::with_capacity(16 * 1024);

    loop {
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 { break; }

        loop {
            if buf.len() < 4 { break; }
            let payload_len = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
            if buf.len() < 4 + payload_len { break; }

            // Start timing when a full frame is available
            let t0 = Instant::now();

            let mut frame = buf.split_to(4 + payload_len);
            frame.advance(4);

            if frame.len() < 4 {
                // malformed
                continue;
            }

            let msg_type = frame.get_u16_le();
            let body_len = frame.get_u16_le() as usize;
            if frame.len() < body_len {
                // malformed
                continue;
            }
            let _body = frame.split_to(body_len);

            match msg_type {
                MSG_PING => ack(&mut socket, b"pong").await?,
                _ => ack(&mut socket, b"").await?,
            }

            // Stop timer ONLY after ACK write completes; send micros to metrics task
            let dt = t0.elapsed().as_micros() as u64;
            // best-effort (ignore send error if shutting down)
            let _ = lat_tx.send(dt);
        }
    }
    Ok(())
}

/// Single background task that aggregates latencies and prints p50/p95/p99 every `period_s`.
async fn spawn_latency_reporter(
    mut lat_rx: mpsc::UnboundedReceiver<u64>,
    period_s: u64,
) {
    use hdrhistogram::Histogram;
    // Track 1 microsecond .. 10 seconds, 3 significant digits
    let mut hist = Histogram::<u64>::new_with_bounds(1, 10_000_000, 3).unwrap();
    let mut tick = interval(Duration::from_secs(period_s));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            // receive latencies continuously
            Some(v) = lat_rx.recv() => {
                // v is in microseconds
                let _ = hist.record(v);
            }
            // periodic report
            _ = tick.tick() => {
                let count = hist.len();
                if count > 0 {
                    let p50 = hist.value_at_quantile(0.50);
                    let p95 = hist.value_at_quantile(0.95);
                    let p99 = hist.value_at_quantile(0.99);
                    let max = hist.max();
                    let min = hist.min();
                    println!(
                        "[latency {}s] n={} p50={}µs p95={}µs p99={}µs min={}µs max={}µs",
                        period_s, count, p50, p95, p99, min, max
                    );
                    // reset for the next window (rolling intervals)
                    hist.reset();
                } else {
                    println!("[latency {}s] n=0 (no messages)", period_s);
                }
            }
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9000").await?;
    println!("🚀 listening on {}", listener.local_addr()?);

    // Channel: hot path just does `lat_tx.send(micros)` (non-blocking)
    let (lat_tx, lat_rx) = mpsc::unbounded_channel::<u64>();
    // Print every 5 seconds (tune to taste)
    tokio::spawn(spawn_latency_reporter(lat_rx, 5));

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("✅ accepted {addr}");
        let tx = lat_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = process(socket, tx).await {
                eprintln!("💥 {addr} error: {e}");
            }
        });
    }
}
