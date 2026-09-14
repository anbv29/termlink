use clap::Parser;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use termlink::auth;
use termlink::database::{Database, User};
use termlink::error::Result;
use termlink::protocol::{self, ClientMessage, ServerMessage};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct OnlineUser {
    id: u64,
    display_name: String,
    direct_sender: mpsc::UnboundedSender<ServerMessage>,
}

#[derive(Debug, Parser)]
#[command(name = "termlink-server", about = "Run the TermLink chat server")]
struct Args {
    #[arg(long, env = "TERMLINK_BIND", default_value = "127.0.0.1:8080")]
    bind: String,

    #[arg(long, env = "DATABASE_URL")]
    database_url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("termlink=info")),
        )
        .with_target(false)
        .init();
    dotenvy::dotenv().ok();
    let args = Args::parse();
    let database = Database::connect(&args.database_url).await?;
    database.migrate().await?;
    let listener = TcpListener::bind(&args.bind).await?;
    let (messages, _) = broadcast::channel::<ServerMessage>(100);
    let (shutdown, _) = broadcast::channel::<()>(1);
    let users = Arc::new(RwLock::new(HashMap::<String, OnlineUser>::new()));
    let mut tasks = JoinSet::new();
    info!(address = %args.bind, "TermLink server listening");

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, peer) = accepted?;
                let messages = messages.clone();
                let users = users.clone();
                let database = database.clone();
                let shutdown = shutdown.subscribe();
                let session_name = Arc::new(Mutex::new(None::<String>));
                info!(%peer, "client connected");

                tasks.spawn(async move {
                    if let Err(error) = handle_client(
                        stream,
                        messages.clone(),
                        users.clone(),
                        database,
                        shutdown,
                        session_name.clone(),
                    ).await {
                        warn!(%peer, %error, "client connection ended with an error");
                    }

                    let disconnected_name = session_name
                        .lock()
                        .ok()
                        .and_then(|name| name.clone());
                    if let Some(username) = disconnected_name {
                        users.write().await.remove(&username.to_ascii_lowercase());
                        let _ = messages.send(ServerMessage::Notice {
                            message: format!("{username} left #general"),
                        });
                    }
                    info!(%peer, "client disconnected");
                });
            }
            signal = tokio::signal::ctrl_c() => {
                signal?;
                info!("shutdown requested; closing client connections");
                break;
            }
        }
    }

    let _ = shutdown.send(());
    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            error!(%error, "client task ended unexpectedly");
        }
    }
    database.close().await;
    info!("TermLink server stopped cleanly");
    Ok(())
}

async fn handle_client(
    stream: TcpStream,
    messages: broadcast::Sender<ServerMessage>,
    users: Arc<RwLock<HashMap<String, OnlineUser>>>,
    database: Database,
    mut shutdown: broadcast::Receiver<()>,
    session_name: Arc<Mutex<Option<String>>>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut inbox = messages.subscribe();
    let (direct_sender, mut direct_inbox) = mpsc::unbounded_channel::<ServerMessage>();
    let mut current_user: Option<User> = None;

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                let _ = write_message(&mut writer, &ServerMessage::Notice {
                    message: "Server is shutting down".to_owned(),
                }).await;
                break;
            }
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
                                            OnlineUser {
                                                id: user.id,
                                                display_name: requested_name.clone(),
                                                direct_sender: direct_sender.clone(),
                                            },
                                        );
                                        current_user = Some(user);
                                        if let Ok(mut name) = session_name.lock() {
                                            *name = Some(requested_name.clone());
                                        }
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
                                        error!(%error, "database registration failed");
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
                                                OnlineUser {
                                                    id: user.id,
                                                    display_name: display_name.clone(),
                                                    direct_sender: direct_sender.clone(),
                                                },
                                            );
                                            current_user = Some(user);
                                            if let Ok(mut name) = session_name.lock() {
                                                *name = Some(display_name.clone());
                                            }
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
                                        error!(%error, "database login failed");
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
                                match database.save_public_message(user.id, &content).await {
                                    Ok(timestamp) => {
                                        let _ = messages.send(ServerMessage::Chat {
                                            room: protocol::GENERAL_ROOM.to_owned(),
                                            username: user.username.clone(),
                                            content,
                                            timestamp,
                                        });
                                    }
                                    Err(error) => {
                                        error!(%error, "saving public message failed");
                                        write_message(&mut writer, &ServerMessage::Error {
                                            message: "The message could not be saved".to_owned(),
                                        }).await?;
                                    }
                                }
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
                            let mut names = users
                                .read()
                                .await
                                .values()
                                .map(|user| user.display_name.clone())
                                .collect::<Vec<_>>();
                            names.sort_by_key(|name| name.to_ascii_lowercase());
                            write_message(&mut writer, &ServerMessage::UserList { users: names }).await?;
                        }
                        Ok(ClientMessage::DirectMessage { to, content }) => {
                            if current_user.is_none() {
                                write_message(&mut writer, &ServerMessage::Error {
                                    message: "Log in before sending private messages".to_owned(),
                                }).await?;
                            } else if let Err(message) = protocol::validate_username(&to)
                                .and_then(|_| protocol::validate_message(&content))
                            {
                                write_message(&mut writer, &ServerMessage::Error { message }).await?;
                            } else {
                                let target = users
                                    .read()
                                    .await
                                    .get(&to.to_ascii_lowercase())
                                    .cloned();
                                if let Some(target) = target {
                                    let sender = current_user.as_ref().expect("checked above");
                                    match database
                                        .save_direct_message(sender.id, target.id, &content)
                                        .await
                                    {
                                        Ok(timestamp) => {
                                            let direct_message = ServerMessage::DirectMessage {
                                                from: sender.username.clone(),
                                                to: target.display_name.clone(),
                                                content,
                                                timestamp,
                                            };
                                            if target.id != sender.id
                                                && target.direct_sender.send(direct_message.clone()).is_err()
                                            {
                                                write_message(&mut writer, &ServerMessage::Error {
                                                    message: "That user is no longer available".to_owned(),
                                                }).await?;
                                            } else {
                                                write_message(&mut writer, &direct_message).await?;
                                            }
                                        }
                                        Err(error) => {
                                            error!(%error, "saving private message failed");
                                            write_message(&mut writer, &ServerMessage::Error {
                                                message: "The private message could not be saved".to_owned(),
                                            }).await?;
                                        }
                                    }
                                } else {
                                    write_message(&mut writer, &ServerMessage::Error {
                                        message: "That user is not online".to_owned(),
                                    }).await?;
                                }
                            }
                        }
                        Ok(ClientMessage::History { limit }) => {
                            match (current_user.as_ref(), protocol::validate_history_limit(limit)) {
                                (None, _) => {
                                    write_message(&mut writer, &ServerMessage::Error {
                                        message: "Log in before requesting history".to_owned(),
                                    }).await?;
                                }
                                (_, Err(message)) => {
                                    write_message(&mut writer, &ServerMessage::Error { message }).await?;
                                }
                                (Some(_), Ok(limit)) => match database.public_history(limit).await {
                                    Ok(history) => {
                                        write_message(&mut writer, &ServerMessage::History {
                                            messages: history,
                                        }).await?;
                                    }
                                    Err(error) => {
                                        error!(%error, "loading public history failed");
                                        write_message(&mut writer, &ServerMessage::Error {
                                            message: "History is temporarily unavailable".to_owned(),
                                        }).await?;
                                    }
                                }
                            }
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
                        warn!(skipped, "slow client missed broadcast messages");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            direct = direct_inbox.recv() => {
                if let Some(message) = direct {
                    write_message(&mut writer, &message).await?;
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
