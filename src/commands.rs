//! Parsing for commands typed in the terminal client.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Users,
    DirectMessage { username: String, message: String },
    History { limit: Option<u32> },
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedInput {
    Chat(String),
    Command(Command),
}

pub fn parse_input(input: &str) -> Result<ParsedInput, String> {
    let input = input.trim();
    if !input.starts_with('/') {
        return Ok(ParsedInput::Chat(input.to_owned()));
    }

    let mut parts = input.split_whitespace();
    let name = parts.next().unwrap_or_default();

    match name {
        "/help" if parts.next().is_none() => Ok(ParsedInput::Command(Command::Help)),
        "/users" if parts.next().is_none() => Ok(ParsedInput::Command(Command::Users)),
        "/quit" if parts.next().is_none() => Ok(ParsedInput::Command(Command::Quit)),
        "/history" => {
            let limit = parts
                .next()
                .map(|value| {
                    value
                        .parse::<u32>()
                        .map_err(|_| "History count must be a positive number".to_owned())
                })
                .transpose()?;
            if parts.next().is_some() {
                return Err("Usage: /history [number]".to_owned());
            }
            Ok(ParsedInput::Command(Command::History { limit }))
        }
        "/dm" => {
            let username = parts
                .next()
                .ok_or_else(|| "Usage: /dm <username> <message>".to_owned())?;
            let message = parts.collect::<Vec<_>>().join(" ");
            if message.is_empty() {
                return Err("Usage: /dm <username> <message>".to_owned());
            }
            Ok(ParsedInput::Command(Command::DirectMessage {
                username: username.to_owned(),
                message,
            }))
        }
        _ => Err(format!("Unknown command: {name}. Try /help")),
    }
}
