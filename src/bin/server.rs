use chrono::Utc;
use clap::Parser;
use std::collections::HashMap;
use std::sync::Arc;
use termlink::database::Database;
use termlink::protocol::{self, ClientMessage, ServerMessage};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast};

#[derive(Debug, Parser)]
#[command(name = "termlink-server", about = "Run the TermLink chat server")]
struct Args {
    #[arg(long, env = "TERMLINK_BIND", default_value = "127.0.0.1:8080")]
    bind: String,

    #[arg(long, env = "DATABASE_URL")]
    database_url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let args = Args::parse();
    let _database = Database::connect(&args.database_url).await?;
    let listener = TcpListener::bind(&args.bind).await?;
    let (messages, _) = broadcast::channel::<ServerMessage>(100);
    let users = Arc::new(RwLock::new(HashMap::<String, String>::new()));
    println!("{} server listening on {}", termlink::APP_NAME, args.bind);

    loop {
        let (stream, peer) = listener.accept().await?;
        let messages = messages.clone();
        let users = users.clone();
        println!("Client connected from {peer}");

        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, messages.clone(), users).await {
                eprintln!("Connection error for {peer}: {error}");
            }
            println!("Client disconnected: {peer}");
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    messages: broadcast::Sender<ServerMessage>,
    users: Arc<RwLock<HashMap<String, String>>>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut inbox = messages.subscribe();
    let mut username: Option<String> = None;

    loop {
        tokio::select! {
            incoming = lines.next_line() => {
                match incoming? {
                    Some(line) if line.len() > protocol::MAX_WIRE_LINE_LENGTH => {
                        write_message(&mut writer, &ServerMessage::Error {
                            message: "Protocol line is too long".to_owned(),
                        }).await?;
                    }
                    Some(line) => match protocol::decode::<ClientMessage>(&line) {
                        Ok(ClientMessage::Join { username: requested_name }) => {
                            if username.is_some() {
                                write_message(&mut writer, &ServerMessage::Error {
                                    message: "A username is already set for this connection".to_owned(),
                                }).await?;
                            } else if let Err(message) = protocol::validate_username(&requested_name) {
                                write_message(&mut writer, &ServerMessage::Error { message }).await?;
                            } else {
                                let key = requested_name.to_ascii_lowercase();
                                let mut online = users.write().await;
                                if online.contains_key(&key) {
                                    drop(online);
                                    write_message(&mut writer, &ServerMessage::Error {
                                        message: "That username is already online".to_owned(),
                                    }).await?;
                                } else {
                                    online.insert(key, requested_name.clone());
                                    drop(online);
                                    username = Some(requested_name.clone());
                                    let _ = messages.send(ServerMessage::Notice {
                                        message: format!("{requested_name} joined #general"),
                                    });
                                }
                            }
                        }
                        Ok(ClientMessage::Chat { content }) => {
                            if let Err(message) = protocol::validate_message(&content) {
                                write_message(&mut writer, &ServerMessage::Error { message }).await?;
                            } else if let Some(username) = &username {
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
                            let mut names = users.read().await.values().cloned().collect::<Vec<_>>();
                            names.sort_by_key(|name| name.to_ascii_lowercase());
                            write_message(&mut writer, &ServerMessage::UserList { users: names }).await?;
                        }
                        Ok(ClientMessage::DirectMessage { to, content }) => {
                            let validation = protocol::validate_username(&to)
                                .and_then(|_| protocol::validate_message(&content));
                            let message = validation.err().unwrap_or_else(|| "Private messages are not available yet".to_owned());
                            write_message(&mut writer, &ServerMessage::Error { message }).await?;
                        }
                        Ok(ClientMessage::History { limit }) => {
                            let message = protocol::validate_history_limit(limit)
                                .err()
                                .unwrap_or_else(|| "Persistent history is not available yet".to_owned());
                            write_message(&mut writer, &ServerMessage::Error { message }).await?;
                        }
                        Ok(ClientMessage::Quit) => break,
                        Err(error) => {
                            write_message(&mut writer, &ServerMessage::Error {
                                message: format!("Malformed JSON message: {error}"),
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
        users.write().await.remove(&username.to_ascii_lowercase());
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
