# TermLink

TermLink is a beginner-friendly terminal chat application written in Rust. It is
being built in small, tested phases so each networking concept is easy to follow.

## Phase 1 status

The first phase contains two programs:

- `server` listens on `127.0.0.1:8080`, accepts one TCP connection, and echoes
  each line it receives.
- `client` connects to that address, sends lines typed in the terminal, and
  prints replies from the server.

TCP provides a reliable stream of bytes. TermLink currently treats each newline
as the boundary between messages. Later phases will replace plain text with
newline-delimited JSON and allow many users to chat at once.

```text
Terminal input -> client -> TCP connection -> server
                       <- echoed response <-
```

## Prerequisites

- Rust and Cargo
- Windows PowerShell or another terminal

## Run Phase 1

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

The next phase will introduce concurrent connections and public broadcasts.
