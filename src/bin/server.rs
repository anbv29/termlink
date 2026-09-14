use chrono::Utc;
use clap::Parser;
use std::collections::HashMap;
use std::sync::Arc;
use termlink::auth;
use termlink::database::{Database, User};
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
    let database = Database::connect(&args.database_url).await?;
    database.migrate().await?;
    let listener = TcpListener::bind(&args.bind).await?;
    let (messages, _) = broadcast::channel::<ServerMessage>(100);
    let users = Arc::new(RwLock::new(HashMap::<String, String>::new()));
    println!("{} server listening on {}", termlink::APP_NAME, args.bind);

    loop {
        let (stream, peer) = listener.accept().await?;
        let messages = messages.clone();
        let users = users.clone();
        let database = database.clone();
        println!("Client connected from {peer}");

        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, messages.clone(), users, database).await {
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
    database: Database,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut inbox = messages.subscribe();
    let mut current_user: Option<User> = None;

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
                        Ok(ClientMessage::Register { username: requested_name, password }) => {
                            if current_user.is_some() {
                                write_message(&mut writer, &ServerMessage::Error {
                                    message: "A username is already set for this connection".to_owned(),
                                }).await?;
                            } else if let Err(message) = protocol::validate_username(&requested_name) {
                                write_message(&mut writer, &ServerMessage::Error { message }).await?;
                            } else if let Err(message) = auth::validate_password(&password) {
                                write_message(&mut writer, &ServerMessage::Error { message }).await?;
                            } else {
                                let password_hash = tokio::task::spawn_blocking(move || {
                                    auth::hash_password(&password)
                                })
                                .await
                                .map_err(std::io::Error::other)?
                                .map_err(|error| std::io::Error::other(error.to_string()))?;

                                match database.create_user(&requested_name, &password_hash).await {
                                    Ok(user) => {
                                        users.write().await.insert(
                                            requested_name.to_ascii_lowercase(),
                                            requested_name.clone(),
                                        );
                                        current_user = Some(user);
                                        write_message(
                                            &mut writer,
                                            &ServerMessage::Authenticated {
                                                username: requested_name.clone(),
                                            },
                                        )
                                        .await?;
                                        let _ = messages.send(ServerMessage::Notice {
                                            message: format!("{requested_name} joined #general"),
                                        });
                                    }
                                    Err(error) if Database::is_duplicate(&error) => {
                                        write_message(&mut writer, &ServerMessage::Error {
                                            message: "That username is already registered".to_owned(),
                                        })
                                        .await?;
                                    }
                                    Err(error) => {
                                        eprintln!("Database registration error: {error}");
                                        write_message(&mut writer, &ServerMessage::Error {
                                            message: "The database could not create the account".to_owned(),
                                        })
                                        .await?;
                                    }
                                }
                            }
                        }
                        Ok(ClientMessage::Login { username: requested_name, password }) => {
                            if current_user.is_some() {
                                write_message(&mut writer, &ServerMessage::Error {
                                    message: "This connection is already authenticated".to_owned(),
                                }).await?;
                            } else if protocol::validate_username(&requested_name).is_err()
                                || auth::validate_password(&password).is_err()
                            {
                                write_message(&mut writer, &ServerMessage::Error {
                                    message: "Invalid username or password".to_owned(),
                                }).await?;
                            } else if users.read().await.contains_key(&requested_name.to_ascii_lowercase()) {
                                write_message(&mut writer, &ServerMessage::Error {
                                    message: "That user is already online".to_owned(),
                                }).await?;
                            } else {
                                match database.user_credentials(&requested_name).await {
                                    Ok(Some(credentials)) => {
                                        let encoded_hash = credentials.password_hash;
                                        let password_matches = tokio::task::spawn_blocking(move || {
                                            auth::verify_password(&password, &encoded_hash)
                                        })
                                        .await
                                        .map_err(std::io::Error::other)?;

                                        if password_matches {
                                            let user = credentials.user;
                                            let display_name = user.username.clone();
                                            users.write().await.insert(
                                                display_name.to_ascii_lowercase(),
                                                display_name.clone(),
                                            );
                                            current_user = Some(user);
                                            write_message(
                                                &mut writer,
                                                &ServerMessage::Authenticated {
                                                    username: display_name.clone(),
                                                },
                                            )
                                            .await?;
                                            let _ = messages.send(ServerMessage::Notice {
                                                message: format!("{display_name} joined #general"),
                                            });
                                        } else {
                                            write_message(&mut writer, &ServerMessage::Error {
                                                message: "Invalid username or password".to_owned(),
                                            }).await?;
                                        }
                                    }
                                    Ok(None) => {
                                        write_message(&mut writer, &ServerMessage::Error {
                                            message: "Invalid username or password".to_owned(),
                                        }).await?;
                                    }
                                    Err(error) => {
                                        eprintln!("Database login error: {error}");
                                        write_message(&mut writer, &ServerMessage::Error {
                                            message: "The database could not complete login".to_owned(),
                                        }).await?;
                                    }
                                }
                            }
                        }
                        Ok(ClientMessage::Chat { content }) => {
                            if let Err(message) = protocol::validate_message(&content) {
                                write_message(&mut writer, &ServerMessage::Error { message }).await?;
                            } else if let Some(user) = &current_user {
                                let _ = messages.send(ServerMessage::Chat {
                                    room: protocol::GENERAL_ROOM.to_owned(),
                                    username: user.username.clone(),
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

    if let Some(user) = current_user {
        users
            .write()
            .await
            .remove(&user.username.to_ascii_lowercase());
        let _ = messages.send(ServerMessage::Notice {
            message: format!("{} left #general", user.username),
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
