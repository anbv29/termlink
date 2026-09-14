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

enum AuthChoice {
    Register,
    Login,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let (choice, username, password) = read_credentials()?;

    let stream = TcpStream::connect(&args.address).await?;
    println!("Connected to {} at {}", termlink::APP_NAME, args.address);
    println!("Type a message and press Enter. Press Ctrl+Z, then Enter, to exit.");

    let (reader, mut writer) = stream.into_split();
    let mut server_lines = BufReader::new(reader).lines();
    let mut input_lines = BufReader::new(io::stdin()).lines();

    let authentication = match choice {
        AuthChoice::Register => ClientMessage::Register { username, password },
        AuthChoice::Login => ClientMessage::Login { username, password },
    };
    let join = protocol::encode(&authentication).map_err(std::io::Error::other)?;
    writer.write_all(join.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    let Some(authentication_response) = server_lines.next_line().await? else {
        eprintln!("Server closed the connection during authentication");
        return Ok(());
    };
    match protocol::decode::<ServerMessage>(&authentication_response) {
        Ok(ServerMessage::Authenticated { username }) => {
            println!("Authenticated as {username}");
        }
        Ok(ServerMessage::Error { message }) => {
            eprintln!("Authentication failed: {message}");
            return Ok(());
        }
        Ok(ServerMessage::Notice { message }) => {
            eprintln!("Server notice during authentication: {message}");
            return Ok(());
        }
        Ok(_) | Err(_) => {
            eprintln!("Server returned an unexpected authentication response");
            return Ok(());
        }
    }

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                let json = protocol::encode(&ClientMessage::Quit)
                    .map_err(std::io::Error::other)?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.shutdown().await?;
                println!("Disconnecting from TermLink...");
                break;
            }
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
                        Ok(ServerMessage::History { messages }) => {
                            if messages.is_empty() {
                                println!("No public messages have been saved yet.");
                            } else {
                                println!("Recent #general messages:");
                                for message in messages {
                                    println!(
                                        "[{}] {}: {}",
                                        message.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
                                        message.username,
                                        message.content
                                    );
                                }
                            }
                        }
                        Ok(ServerMessage::DirectMessage { from, to, content, timestamp }) => {
                            println!(
                                "[{}] DM {from} -> {to}: {content}",
                                timestamp.format("%H:%M:%S")
                            );
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

fn read_credentials() -> std::io::Result<(AuthChoice, String, String)> {
    let choice = loop {
        print!("Register or login? [r/l]: ");
        std::io::stdout().flush()?;
        let mut choice = String::new();
        std::io::stdin().read_line(&mut choice)?;
        match choice.trim().to_ascii_lowercase().as_str() {
            "r" | "register" => break AuthChoice::Register,
            "l" | "login" => break AuthChoice::Login,
            _ => eprintln!("Enter r to register or l to log in"),
        }
    };

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
                    Ok(()) => return Ok((choice, username, password)),
                    Err(error) => eprintln!("{error}"),
                }
            }
            Err(error) => eprintln!("{error}"),
        }
    }
}
