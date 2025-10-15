use tokio::{net::TcpListener};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:9000").await?;
    println!("Listening on {}", listener.local_addr()?);
    Ok(())
}