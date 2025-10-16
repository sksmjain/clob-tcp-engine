use tokio::{
    io::{BufReader, AsyncBufReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const MSG_PING: u16 = 1;
const MSG_ACK:  u16 = 100;

async fn process_lines(socket: TcpStream) {
    let (r, mut w) = socket.into_split();
    let mut lines = BufReader::new(r).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let _ = w.write_all(format!("ACK: {line}\n").as_bytes()).await;
    }
}

async fn process(socket: TcpStream) {
   let mut buf = ButesMut::with_capacity(16 * 1024);
   loop {
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 {break;} // peer closed the connection

        while buf.len() >= 6 {
            let len = u32::from_le_bytes(buf[0..4]).try_into().unwrap() as usize;
            if buf.len() < len + 4 {break;}

            let mut frame = buf.split_to(4 + len).freeze();
            let cmd = u32::from_le_bytes(frame[0..4].try_into().unwrap());
            frame.advance(4); // drop length

            let msg_type = u16::from_le_bytes(frame.get_u8(), frame.get_u8());
            let payload = frame; // remaining bytes

            match msg_type {
                MSG_PING => {
                    let _ = payload; // none for ping
                    ack(&mut socket, b"pong").await?;
                }
                _ => ack(&mut socket, b"").await?;
            }
        }
   }
}


/// send: [u32 len][u16 MSG_ACK][u16 body_len][body…]
async fn ack(sock: &mut TcpStream, body: &[u8]) -> Result<()> {
    let total = 2 + 2 + body.len();
    let mut out = BytesMut::with_capacity(4 + total);
    out.put_u32_le(total as u32);
    out.put_u16_le(MSG_ACK);
    out.put_u16_le(body.len() as u16);
    out.extend_from_slice(body);
    sock.write_all(&out).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9000").await?;
    println!("Listening on {}", listener.local_addr()?);

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("Accepted connection from client: {}", addr);

        tokio::spawn(async move{
            // Process each socket concurrently
            if let Err(e) = process(socket).await {
                eprintln!("Error processing socket: {e}");
            };
        });
    }
}