use crossbeam::channel::{bounded, Receiver, Sender};

// ========================== Protocol ==========================

const MSG_PING: u16 = 1;
const MSG_NEW_ORDER: u16 = 10;
const MSG_CANCEL: u16 = 11;

// Frame: [u32 len][u16 type][payload...]

// ========================== Task Process ==========================

async fn process(mut socket: TcpStream,
    tx_cmd: Sender<Command>,
    sink_to_engine: Sender<Event>,
    rx_evt: Receiver<Event>) -> anyhow::Result<()> {

    socket.set_nodelay(true)?;
    let mut buf = BytesMut::with_capacity(16 * 1024);

    loop {
        // 1) Read any inbound bytes
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 {
            break; // peer closed
        }

        // 2) Parse all complete frames
        while buf.len() >= 6 {
            let len = u32::from_le_bytes(buf[0..4].try_into().unwrap() as usize);
            if buf.len() < 4 + len {
                break; // incomplete
            }
            let mut frame = buf.split_to(4 + payload_len);
            frame.advance(4);

            let msg_type = frame.get_u16_le();
            let body_len = frame.get_u16_le() as usize;
            let _body = frame.split_to(body_len);

            match msg_type {
                MSG_PING => {

                }
                MSG_NEW_ORDER => {

                }
                MSG_CANCEL => {

                }
                _ => {

                }
            }
        }
    }

    Ok(())
}

// ========================== Gateway (Tokio async TCP) ==========================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("ADDR").unwrap_or_else(|_| "0.0.0.0:9000".to_string());
    let listener = TcpListener::bind(&addr).await?;
    println!("🚀 listening on {}", listener.local_addr()?);

    // Engine thread channel
    // Use bounded channel for backpressure protection
    // tx_cmd: Used by our async TCP handlers to transmit new orders or cancels to the engine.
    // rx_cmd: Owned by the engine thread; it receives one Command at a time.
    let (tx_cmd, rx_cmd) = bounded::<Command>(10_000);
    let (tx_bcast, _rx_bcast) = bounded::<Event>(10_000);

    // Spawn engine on its own OS thread
    thread::spawn(move || run_engine(rx_cmd, tx_bcast));

    // Accept loop
    loop {
        let (socket, peer) = listener.accept().await?;
        println!("✅ client connected: {peer}");

        let tx_cmd_cl = tx_cmd.clone();

        // Per-connection event sink
        let (tx_evt, rx_evt) = bounded::<Event>(2048);

        tokio::spawn(async move {
            if let Err(e) = process(socket, tx_cmd_cl, tx_evt, rx_evt).await {
                error!("conn error: {e:#}");
            }
            info!("client disconnected: {peer}");
        })

    }
}