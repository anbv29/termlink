use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const ADDRESS: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDRESS).await?;
    println!("{} server listening on {ADDRESS}", termlink::APP_NAME);

    let (mut stream, peer) = listener.accept().await?;
    println!("Client connected from {peer}");

    let (reader, mut writer) = stream.split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    println!("Client disconnected");
    Ok(())
}
