use clap::Parser;
use std::io::Write as _;
use termlink::commands::{self, Command, ParsedInput};
use termlink::protocol::{self, ClientMessage, ServerMessage};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Debug, Parser)]
#[command(name = "termlink-client", about = "Connect to a TermLink server")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8080")]
    address: String,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let (username, password) = read_registration()?;

    let stream = TcpStream::connect(&args.address).await?;
    println!("Connected to {} at {}", termlink::APP_NAME, args.address);
    println!("Type a message and press Enter. Press Ctrl+Z, then Enter, to exit.");

    let (reader, mut writer) = stream.into_split();
    let mut server_lines = BufReader::new(reader).lines();
    let mut input_lines = BufReader::new(io::stdin()).lines();

    let join = protocol::encode(&ClientMessage::Register { username, password })
        .map_err(std::io::Error::other)?;
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
                        Ok(ServerMessage::Authenticated { username }) => {
                            println!("Authenticated as {username}");
                        }
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

fn read_registration() -> std::io::Result<(String, String)> {
    loop {
        print!("Username: ");
        std::io::stdout().flush()?;
        let mut username = String::new();
        std::io::stdin().read_line(&mut username)?;
        let username = username.trim().to_owned();
        match protocol::validate_username(&username) {
            Ok(()) => {
                print!("Password: ");
                std::io::stdout().flush()?;
                let mut password = String::new();
                std::io::stdin().read_line(&mut password)?;
                let password = password.trim_end().to_owned();
                match termlink::auth::validate_password(&password) {
                    Ok(()) => return Ok((username, password)),
                    Err(error) => eprintln!("{error}"),
                }
            }
            Err(error) => eprintln!("{error}"),
        }
    }
}
