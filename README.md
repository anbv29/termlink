# TermLink

TermLink is a beginner-friendly terminal chat application written in Rust. It is
being built in small, tested phases so each networking concept is easy to follow.

## Phase 4 status

The project currently contains two programs:

- `server` listens on `127.0.0.1:8080`, creates one lightweight Tokio task per
  connection, and broadcasts each received line to every connected client.
- `client` connects to that address, sends lines typed in the terminal, and
  prints replies from the server.

TCP provides a reliable stream of bytes. TermLink treats each newline as the
boundary between JSON messages. Serde converts typed Rust enums to JSON and back,
so malformed or unexpected input can be reported instead of crashing the server.
A Tokio broadcast channel gives every connection its own receiver.

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

The next phase will add private messaging, graceful shutdown, richer logging,
and final hardening.

Phase 4 now stores accounts and public messages in MySQL. Passwords are converted
to salted Argon2id hashes before insertion; the plain password is never written to
the database. `/history` requests at most 100 recent messages and displays them in
chronological order.

Copy `.env.example` to `.env`, create the database and application user described
by its `DATABASE_URL`, then start the server. The server applies migrations before
accepting connections. Full setup commands are added in the final documentation.

## Current chat commands

- `/help` lists available commands.
- `/users` lists connected usernames.
- `/dm <username> <message>` is parsed now and will be enabled in Phase 5.
- `/history [number]` retrieves recent public messages from MySQL.
- `/quit` closes the client connection.

Usernames contain 3–24 ASCII letters, numbers, underscores, or hyphens. Chat
messages contain 1–1,000 Unicode characters. The server checks these rules even
when a custom client bypasses the official terminal client.
