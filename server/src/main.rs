use tokio::{
    io::AsyncReadExt,
    net::{TcpStream, TcpListener}
};
use crossbeam::channel::{bounded, Receiver, Sender};
use bytes::{BytesMut, Buf};
use tracing::{error, info};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

mod types;
mod engine;
use crate::types::{Command, Event, Order, Side, Tif};
use crate::engine::run_engine;

use tracing_appender::rolling;
use tracing_subscriber::fmt;

// ========================== Protocol ==========================
// Frame: [u32 len][u16 type][u16 body_len][payload...]
// Side encoding: 0 = BID, 1 = ASK

const MSG_PING: u16 = 1;
const MSG_NEW_ORDER: u16 = 10;
const MSG_CANCEL: u16 = 11;

// ========================== Task Process ==========================

async fn process(
    mut socket: TcpStream,
    tx_cmd: Sender<Command>,
    sink_to_engine: Sender<Event>,
    rx_evt: Receiver<Event>,
) -> anyhow::Result<()> {
    socket.set_nodelay(true)?;
    let peer_addr = socket.peer_addr()?;
    println!("🟢 [CONNECT] New client: {peer_addr}");

    let mut buf = BytesMut::with_capacity(16 * 1024);

    loop {
        // 1️⃣ Read inbound bytes
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 {
            println!("🔴 [DISCONNECT] Client closed connection: {peer_addr}");
            break;
        }

        println!("\n📥 [RECV] {} bytes from {}", n, peer_addr);
        println!("🧩 Raw buffer (hex): {}", hex::encode(&buf));

        // 2️⃣ Parse complete frames
        while buf.len() >= 6 {
            let payload_len = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;

            if buf.len() < 4 + payload_len {
                println!(
                    "⚠️ [WAIT] Incomplete frame: have {} bytes, need {} bytes",
                    buf.len(),
                    4 + payload_len
                );
                break;
            }

            // Extract full frame
            let mut frame = buf.split_to(4 + payload_len);
            frame.advance(4); // skip len prefix

            let msg_type = frame.get_u16_le();
            let body_len = frame.get_u16_le() as usize;

            // Get payload body
            let body = frame.split_to(body_len);
            let body_hex = hex::encode(&body);
            // println!(
            //     "\n🔎 [FRAME DECODED]
            //         • msg_type: {} ({})
            //         • body_len: {}
            //         • raw_body (hex): {}",
            //     msg_type,
            //     match msg_type {
            //         MSG_PING => "PING",
            //         MSG_NEW_ORDER => "NEW_ORDER",
            //         MSG_CANCEL => "CANCEL",
            //         _ => "UNKNOWN",
            //     },
            //     body_len,
            //     body_hex
            // );

            // Decode payload meaningfully if known type
            match msg_type {
                MSG_PING => {
                    // println!("💓 [PING] Received ping from {}", peer_addr);
                    // forward to engine so it can respond
                    if let Err(e) = tx_cmd.send(Command::Ping(sink_to_engine.clone())) {
                        eprintln!("[gw] failed to send Ping to engine: {e}");
                    }
                }

                MSG_NEW_ORDER => {
                    // println!("🟦 [NEW_ORDER] Raw payload len={}", body_len);
                    if body_len >= (8 + 8 + 1 + 8 + 8 + 1) {
                        let client_id = u64::from_le_bytes(body[0..8].try_into().unwrap());
                        let cl_ord_id = u64::from_le_bytes(body[8..16].try_into().unwrap());
                        let side = body[16]; // 0=BID, 1=ASK
                        let price = i64::from_le_bytes(body[17..25].try_into().unwrap());
                        let qty = i64::from_le_bytes(body[25..33].try_into().unwrap());
                        let tif = body[33];
                        let order = Order {
                            id: cl_ord_id,
                            cl_id: client_id,
                            side: if side == 0 { Side::Bid } else { Side::Ask },
                            price: u64::try_from(price).expect("price must be >= 0"),
                            qty: u64::try_from(qty).expect("qty must be >= 0"),
                            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                            tif: if tif == 0 { Tif::Gtc } else { Tif::Ioc },
                        };
            
                        if let Err(e) = tx_cmd.send(Command::Order(order, sink_to_engine.clone())) {
                            eprintln!("[gw] failed to send Order to engine: {e}");
                        }
                    } else {
                        println!("⚠️ [NEW_ORDER] Unexpected payload length: {}", body_len);
                    }
                }

                MSG_CANCEL => {
                    if body_len >= 16 {
                        let client_id = u64::from_le_bytes(body[0..8].try_into().unwrap());
                        let cl_ord_id = u64::from_le_bytes(body[8..16].try_into().unwrap());
                        // println!(
                        //     "🟧 [CANCEL]
                        //     → client_id: {}
                        //     → cl_ord_id: {}",
                        //     client_id, cl_ord_id
                        // );
                    } else {
                        println!("⚠️ [CANCEL] Invalid payload length: {}", body_len);
                    }
                }

                _ => {
                    println!("❓ [UNKNOWN] Message type {} from {}", msg_type, peer_addr);
                }
            }
            println!("----------------------------------------------------------------------")
        }
    }

    Ok(())
}

// ========================== Gateway (Tokio async TCP) ==========================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing setup
    let file_appender = rolling::hourly("logs", "engine.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_target(false)
        .with_level(true)
        .compact()
        .init();
    // Bind address
    let addr = std::env::var("ADDR").unwrap_or_else(|_| "0.0.0.0:9000".to_string());
    let listener = TcpListener::bind(&addr).await?;
    println!("\n🚀 Listening on {}\n", listener.local_addr()?);

    // Engine setup
    let (tx_cmd, rx_cmd) = bounded::<Command>(10_000);
    let (tx_bcast, _rx_bcast) = bounded::<Event>(10_000);

    println!("⚙️  Spawning matching engine thread ...");
    thread::spawn(move || run_engine(rx_cmd, tx_bcast));
    println!("✅ Engine thread started.\n");

    // Accept loop
    loop {
        let (socket, peer) = listener.accept().await?;
        println!("🔗 [ACCEPT] Client connected: {peer}");

        let tx_cmd_cl = tx_cmd.clone();
        let (tx_evt, rx_evt) = bounded::<Event>(2048);

        tokio::spawn(async move {
            if let Err(e) = process(socket, tx_cmd_cl, tx_evt, rx_evt).await {
                error!("❌ [ERROR] {e:#}");
            }
            info!("🔚 [CLOSE] Client {peer} disconnected.");
        });
    }
}
