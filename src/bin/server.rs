use chrono::Utc;
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
            if let Err(error) = handle_client(stream, messages.clone()).await {
                eprintln!("Connection error for {peer}: {error}");
            }
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
    let mut username: Option<String> = None;

    loop {
        tokio::select! {
            incoming = lines.next_line() => {
                match incoming? {
                    Some(line) => match protocol::decode::<ClientMessage>(&line) {
                        Ok(ClientMessage::Join { username: requested_name }) => {
                            if username.is_some() {
                                write_message(&mut writer, &ServerMessage::Error {
                                    message: "A username is already set for this connection".to_owned(),
                                }).await?;
                            } else {
                                username = Some(requested_name.clone());
                                let _ = messages.send(ServerMessage::Notice {
                                    message: format!("{requested_name} joined #general"),
                                });
                            }
                        }
                        Ok(ClientMessage::Chat { content }) => {
                            if let Some(username) = &username {
                                let _ = messages.send(ServerMessage::Chat {
                                    room: protocol::GENERAL_ROOM.to_owned(),
                                    username: username.clone(),
                                    content,
                                    timestamp: Utc::now(),
                                });
                            } else {
                                write_message(&mut writer, &ServerMessage::Error {
                                    message: "Choose a username before chatting".to_owned(),
                                }).await?;
                            }
                        }
                        Ok(ClientMessage::Help) => {
                            write_message(&mut writer, &ServerMessage::Notice {
                                message: "Commands: /help, /users, /dm <username> <message>, /history [number], /quit".to_owned(),
                            }).await?;
                        }
                        Ok(ClientMessage::ListUsers) => {
                            write_message(&mut writer, &ServerMessage::Notice {
                                message: "The user list will be available after session tracking is added".to_owned(),
                            }).await?;
                        }
                        Ok(ClientMessage::DirectMessage { .. }) => {
                            write_message(&mut writer, &ServerMessage::Error {
                                message: "Private messages are not available yet".to_owned(),
                            }).await?;
                        }
                        Ok(ClientMessage::History { .. }) => {
                            write_message(&mut writer, &ServerMessage::Error {
                                message: "Persistent history is not available yet".to_owned(),
                            }).await?;
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

    if let Some(username) = username {
        let _ = messages.send(ServerMessage::Notice {
            message: format!("{username} left #general"),
        });
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
