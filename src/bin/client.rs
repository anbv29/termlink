use termlink::protocol::{self, ClientMessage, ServerMessage};
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
                        let message = if line.trim() == "/quit" {
                            ClientMessage::Quit
                        } else {
                            ClientMessage::Chat { content: line }
                        };
                        let json = protocol::encode(&message).map_err(std::io::Error::other)?;
                        writer.write_all(json.as_bytes()).await?;
                        writer.write_all(b"\n").await?;

                        if matches!(message, ClientMessage::Quit) {
                            break;
                        }
                    }
                    None => {
                        writer.shutdown().await?;
                        break;
                    }
                }
            }
            message = server_lines.next_line() => {
                match message? {
                    Some(line) => match protocol::decode::<ServerMessage>(&line) {
                        Ok(ServerMessage::Chat { content }) => println!("{content}"),
                        Ok(ServerMessage::Notice { message }) => println!("* {message}"),
                        Ok(ServerMessage::Error { message }) => eprintln!("Error: {message}"),
                        Err(_) => eprintln!("Received a malformed message from the server"),
                    },
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
