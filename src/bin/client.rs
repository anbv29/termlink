use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const ADDRESS: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let stream = TcpStream::connect(ADDRESS).await?;
    println!("Connected to {} at {ADDRESS}", termlink::APP_NAME);
    println!("Type a message and press Enter. Press Ctrl+Z, then Enter, to exit.");

    let (reader, mut writer) = stream.into_split();
    let mut server_lines = BufReader::new(reader).lines();
    let mut input_lines = BufReader::new(io::stdin()).lines();

    loop {
        tokio::select! {
            input = input_lines.next_line() => {
                match input? {
                    Some(line) => {
                        writer.write_all(line.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    None => {
                        writer.shutdown().await?;
                        break;
                    }
                }
            }
            message = server_lines.next_line() => {
                match message? {
                    Some(line) => println!("Server: {line}"),
                    None => {
                        println!("Server closed the connection.");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
