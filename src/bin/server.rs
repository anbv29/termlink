use termlink::protocol::{self, ClientMessage, ServerMessage};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

const ADDRESS: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDRESS).await?;
    let (messages, _) = broadcast::channel::<ServerMessage>(100);
    println!("{} server listening on {ADDRESS}", termlink::APP_NAME);

    loop {
        let (stream, peer) = listener.accept().await?;
        let messages = messages.clone();
        println!("Client connected from {peer}");

        tokio::spawn(async move {
            let _ = messages.send(ServerMessage::Notice {
                message: format!("{peer} joined the chat"),
            });

            if let Err(error) = handle_client(stream, messages.clone()).await {
                eprintln!("Connection error for {peer}: {error}");
            }

            let _ = messages.send(ServerMessage::Notice {
                message: format!("{peer} left the chat"),
            });
            println!("Client disconnected: {peer}");
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    messages: broadcast::Sender<ServerMessage>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut inbox = messages.subscribe();

    loop {
        tokio::select! {
            incoming = lines.next_line() => {
                match incoming? {
                    Some(line) => match protocol::decode::<ClientMessage>(&line) {
                        Ok(ClientMessage::Chat { content }) => {
                            let _ = messages.send(ServerMessage::Chat { content });
                        }
                        Ok(ClientMessage::Quit) => break,
                        Err(_) => {
                            write_message(&mut writer, &ServerMessage::Error {
                                message: "Malformed JSON message".to_owned(),
                            }).await?;
                        }
                    }
                    None => break,
                }
            }
            broadcast = inbox.recv() => {
                match broadcast {
                    Ok(message) => write_message(&mut writer, &message).await?,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        eprintln!("Slow client skipped {skipped} messages");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    Ok(())
}

async fn write_message<W>(writer: &mut W, message: &ServerMessage) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let json = protocol::encode(message).map_err(std::io::Error::other)?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await
}
