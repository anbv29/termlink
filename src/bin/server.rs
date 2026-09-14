use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

const ADDRESS: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDRESS).await?;
    let (messages, _) = broadcast::channel::<String>(100);
    println!("{} server listening on {ADDRESS}", termlink::APP_NAME);

    loop {
        let (stream, peer) = listener.accept().await?;
        let messages = messages.clone();
        println!("Client connected from {peer}");

        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, messages).await {
                eprintln!("Connection error for {peer}: {error}");
            }
            println!("Client disconnected: {peer}");
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    messages: broadcast::Sender<String>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let _inbox = messages.subscribe();

    while let Some(line) = lines.next_line().await? {
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    Ok(())
}
