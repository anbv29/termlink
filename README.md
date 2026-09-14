# TermLink

TermLink is a beginner-friendly terminal chat application written in Rust. It is
being built in small, tested phases so each networking concept is easy to follow.

## Phase 2 status

The project currently contains two programs:

- `server` listens on `127.0.0.1:8080`, creates one lightweight Tokio task per
  connection, and broadcasts each received line to every connected client.
- `client` connects to that address, sends lines typed in the terminal, and
  prints replies from the server.

TCP provides a reliable stream of bytes. TermLink treats each newline as the
boundary between messages. A Tokio broadcast channel gives every connection its
own receiver, while one shared sender copies public messages to all receivers.
Later phases will replace plain text with newline-delimited JSON.

```text
Client A --TCP--\
                 server -> Tokio broadcast channel -> Client A
Client B --TCP--/                                  -> Client B
```

## Prerequisites

- Rust and Cargo
- Windows PowerShell or another terminal

## Run TermLink

Open two terminals in the project directory. Start the server first:

```powershell
cargo run --bin server
```

Then start the client in the second terminal:

```powershell
cargo run --bin client
```

Type a line in the client and press Enter. The server echoes it back. Stop either
program with `Ctrl+C`. On Windows, terminal input can also be closed with
`Ctrl+Z`, followed by Enter.

## Verify the project

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
```

## Project layout

```text
Cargo.toml          Rust package configuration
src/lib.rs          Code shared by both programs
src/bin/server.rs   TCP server entry point
src/bin/client.rs   Terminal client entry point
tests/              Integration tests
```

The next phase will introduce usernames, timestamps, structured JSON, and chat
commands.
