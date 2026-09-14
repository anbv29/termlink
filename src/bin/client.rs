use std::io::Write as _;
use termlink::commands::{self, Command, ParsedInput};
use termlink::protocol::{self, ClientMessage, ServerMessage};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

const ADDRESS: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let username = read_username()?;

    let stream = TcpStream::connect(ADDRESS).await?;
    println!("Connected to {} at {ADDRESS}", termlink::APP_NAME);
    println!("Type a message and press Enter. Press Ctrl+Z, then Enter, to exit.");

    let (reader, mut writer) = stream.into_split();
    let mut server_lines = BufReader::new(reader).lines();
    let mut input_lines = BufReader::new(io::stdin()).lines();

    let join =
        protocol::encode(&ClientMessage::Join { username }).map_err(std::io::Error::other)?;
    writer.write_all(join.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    loop {
        tokio::select! {
            input = input_lines.next_line() => {
                match input? {
                    Some(line) => {
                        let message = match commands::parse_input(&line) {
                            Ok(ParsedInput::Chat(content)) => ClientMessage::Chat { content },
                            Ok(ParsedInput::Command(command)) => match command {
                                Command::Help => ClientMessage::Help,
                                Command::Users => ClientMessage::ListUsers,
                                Command::DirectMessage { username, message } => {
                                    ClientMessage::DirectMessage { to: username, content: message }
                                }
                                Command::History { limit } => ClientMessage::History { limit },
                                Command::Quit => ClientMessage::Quit,
                            },
                            Err(error) => {
                                eprintln!("{error}");
                                continue;
                            }
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
                        Ok(ServerMessage::Chat { room, username, content, timestamp }) => {
                            println!("[{}] #{room} {username}: {content}", timestamp.format("%H:%M:%S"));
                        }
                        Ok(ServerMessage::Notice { message }) => println!("* {message}"),
                        Ok(ServerMessage::UserList { users }) => {
                            println!("Online users ({}): {}", users.len(), users.join(", "));
                        }
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

fn read_username() -> std::io::Result<String> {
    loop {
        print!("Username: ");
        std::io::stdout().flush()?;
        let mut username = String::new();
        std::io::stdin().read_line(&mut username)?;
        let username = username.trim().to_owned();
        match protocol::validate_username(&username) {
            Ok(()) => return Ok(username),
            Err(error) => eprintln!("{error}"),
        }
    }
}
