use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

const ADDRESS: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDRESS).await?;
    println!("{} server listening on {ADDRESS}", termlink::APP_NAME);

    loop {
        let (stream, peer) = listener.accept().await?;
        println!("Client connected from {peer}");

        tokio::spawn(async move {
            if let Err(error) = handle_client(stream).await {
                eprintln!("Connection error for {peer}: {error}");
            }
            println!("Client disconnected: {peer}");
        });
    }
}

async fn handle_client(stream: TcpStream) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    Ok(())
}
