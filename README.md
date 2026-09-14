# TermLink

TermLink is a beginner-friendly, asynchronous terminal chat application written
in Rust. It runs entirely on one Windows computer and includes two executables:

- `server`: accepts concurrent TCP clients, authenticates users, routes public
  and private messages, and persists data in MySQL.
- `client`: provides an interactive terminal interface for registration, login,
  public chat, direct messages, online users, and message history.

Messages on the network are newline-delimited JSON (NDJSON): every JSON object is
written on one line. The project uses safe Rust and contains no `unsafe` blocks.

## Architecture

```text
                         TermLink server process
                    +--------------------------------+
Terminal client A --| TCP connection task            |
Terminal client B --| TCP connection task            |-- public broadcast channel
Terminal client C --| TCP connection task            |-- online-session registry -- DMs
                    |        |                       |
                    |        +-- Argon2 worker       |
                    |        +-- SQLx connection pool|---- MySQL 8
                    +--------------------------------+
```

Each connection runs as a lightweight Tokio task. Public messages pass through a
broadcast channel, while direct messages use a sender stored only for the target
session. Blocking Argon2 password work runs outside Tokio's asynchronous worker
threads. SQLx manages a reusable pool of MySQL connections.

## Technology

- Rust 2024 edition
- Tokio for asynchronous TCP, tasks, signals, and channels
- Serde and `serde_json` for NDJSON messages
- Clap for command-line options and environment variables
- SQLx with MySQL for migrations and persistence
- Chrono for UTC timestamps
- Tracing for structured server logs
- Argon2id for salted password hashing

## Prerequisites

- Windows 10 or Windows 11
- Rust and Cargo, installed from [rustup](https://rustup.rs/)
- MySQL Community Server 8.0
- Git, if you want to clone or contribute

Check the tools in PowerShell:

```powershell
rustc --version
cargo --version
git --version
& "C:\Program Files\MySQL\MySQL Server 8.0\bin\mysql.exe" --version
```

The MySQL executable can be added to `PATH`, but using its full path also works.

## MySQL setup

Open PowerShell and start the MySQL client as an administrator account:

```powershell
& "C:\Program Files\MySQL\MySQL Server 8.0\bin\mysql.exe" -u root -p
```

At the MySQL prompt, create a database and a dedicated local application user.
Replace the example password before using these commands:

```sql
CREATE DATABASE termlink
    CHARACTER SET utf8mb4
    COLLATE utf8mb4_0900_ai_ci;

CREATE USER 'termlink'@'127.0.0.1'
    IDENTIFIED BY 'ChangeThis_12345';

GRANT SELECT, INSERT, UPDATE, DELETE, CREATE, ALTER, INDEX, REFERENCES
    ON termlink.* TO 'termlink'@'127.0.0.1';

FLUSH PRIVILEGES;
EXIT;
```

TermLink runs its embedded migrations when the server starts. The database must
already exist, but the tables do not need to be created manually.

## Environment configuration

Copy the example file and open the copy in a text editor:

```powershell
Copy-Item .env.example .env
notepad .env
```

Example `.env`:

```dotenv
DATABASE_URL=mysql://termlink:ChangeThis_12345@127.0.0.1:3306/termlink
TERMLINK_BIND=127.0.0.1:8080
RUST_LOG=termlink=info
```

If a password contains URL-reserved characters such as `@`, `:`, `/`, or `%`,
percent-encode those characters in `DATABASE_URL`. The real `.env` is ignored by
Git; never commit it.

Configuration variables:

| Variable | Purpose | Example |
| --- | --- | --- |
| `DATABASE_URL` | MySQL connection URL used by the server | `mysql://user:pass@127.0.0.1:3306/termlink` |
| `TERMLINK_BIND` | Local address on which the server listens | `127.0.0.1:8080` |
| `RUST_LOG` | Tracing filter for server logs | `termlink=info` |

The server also accepts `--bind` and `--database-url`. Command-line values take
precedence over environment values. Avoid putting a real password directly in a
shell command because it may be stored in terminal history.

## Running TermLink

Start the server from the project directory:

```powershell
cargo run --bin server
```

Open a second PowerShell window and start a client:

```powershell
cargo run --bin client
```

Open additional terminal windows and run the same client command to chat as
other users. To connect to a different address:

```powershell
cargo run --bin client -- --address 127.0.0.1:8080
```

The client first asks whether to register or log in. Usernames are
case-insensitively unique and must contain 3–24 ASCII letters, numbers,
underscores, or hyphens. Passwords must contain 8–128 characters. Successful
registration logs the new account in immediately.

Stop a client with `/quit` or `Ctrl+C`. Stop the server with `Ctrl+C`; it notifies
connected clients, drains their tasks, removes online sessions, and closes the
database pool.

## Chat commands

| Command | Description |
| --- | --- |
| `/help` | Show all available commands. |
| `/users` | List authenticated users currently online. |
| `/dm <username> <message>` | Send a private message to an online user. |
| `/history [number]` | Show recent public messages; default 20, maximum 100. |
| `/quit` | Disconnect cleanly. |

Text without a leading command is sent to the public `general` room. Public and
private messages contain 1–1,000 Unicode characters. Unknown, incomplete, or
malformed commands are reported without terminating the client or server.

## Database schema

Migrations live in `migrations/` and create these tables:

### `users`

| Column | Type | Notes |
| --- | --- | --- |
| `id` | `BIGINT UNSIGNED` | Auto-incrementing primary key |
| `username` | `VARCHAR(24)` | Case-insensitive unique value |
| `password_hash` | `VARCHAR(255)` | Salted Argon2id encoded hash |
| `created_at` | `TIMESTAMP(6)` | Account creation time |

### `messages`

| Column | Type | Notes |
| --- | --- | --- |
| `id` | `BIGINT UNSIGNED` | Auto-incrementing primary key |
| `sender_id` | `BIGINT UNSIGNED` | Foreign key to `users.id` |
| `recipient_id` | `BIGINT UNSIGNED NULL` | Target user for a DM; null for public chat |
| `room` | `VARCHAR(32) NULL` | `general` for public chat; null for a DM |
| `content` | `VARCHAR(1000)` | Validated message text |
| `created_at` | `TIMESTAMP(6)` | UTC-oriented message time |

The destination check ensures that a message has either a room or a private
recipient, never both. Indexes support public history and recipient lookups.

## Protocol example

TermLink sends one compact JSON object followed by `\n`. For example:

```json
{"type":"chat","content":"Hello from Rust!"}
```

The corresponding public server event includes identity, room, and timestamp:

```json
{"type":"chat","room":"general","username":"alice","content":"Hello from Rust!","timestamp":"2026-09-14T12:30:00Z"}
```

The shared protocol types are defined in `src/protocol.rs` so the client and
server cannot silently disagree about message fields.

## Example terminal session

Server:

```text
INFO TermLink server listening address=127.0.0.1:8080
INFO client connected peer=127.0.0.1:53120
INFO client connected peer=127.0.0.1:53121
```

Alice's client:

```text
Register or login? [r/l]: r
Username: alice
Password: ********
Connected to TermLink at 127.0.0.1:8080
Authenticated as alice
* bob joined #general
Hello everyone!
[12:30:04] #general alice: Hello everyone!
/dm bob Are you learning Tokio too?
[12:30:09] DM alice -> bob: Are you learning Tokio too?
/history 2
Recent #general messages:
[2026-09-14 12:29:58 UTC] bob: Hi Alice!
[2026-09-14 12:30:04 UTC] alice: Hello everyone!
/quit
```

The current client reads passwords with ordinary terminal input, so actual input
is visible rather than replaced by the illustrative asterisks above.

## Testing and quality checks

Run formatting, compilation, strict linting, and tests:

```powershell
cargo fmt --all -- --check
cargo check --all-targets -j 1
cargo clippy --all-targets -j 1 -- -D warnings
cargo test --all-targets -j 1
```

`-j 1` keeps memory usage predictable on smaller Windows computers. Tests cover
command parsing, username/password/message validation, history bounds, Argon2
hashing and verification, JSON round trips, malformed JSON, broadcast delivery,
and a real NDJSON exchange over a loopback TCP socket.

Database methods use runtime-checked SQLx queries. To verify migrations and live
database behavior, configure `.env`, start MySQL, and run the server.

Some managed Windows installations block newly compiled unsigned test programs
with OS error 4551. This is an Application Control policy, not a Rust test
failure. Do not disable organizational security controls; ask the device
administrator whether local development binaries can be allowed. `cargo check`
and Clippy still verify every target without executing those binaries.

## Error handling

TermLink treats each connection as isolated work. Invalid commands, malformed
JSON, overlong input, duplicate accounts, wrong credentials, offline DM targets,
database failures, slow receivers, and unexpected disconnects produce a client
error or structured server log rather than crashing the accept loop. Private
messages are routed through per-user channels and never broadcast publicly.

## Security limitations

TermLink is an educational local application, not an internet-ready service:

- TCP traffic is not encrypted. Login passwords and chat messages are visible to
  software capable of inspecting local network traffic. Keep the default
  loopback bind address and do not expose the port to other computers.
- Passwords are visible while being typed in the current terminal client.
- Stored passwords use salted Argon2id hashes; plaintext passwords are never
  stored or logged.
- There is no login rate limit, account lockout, password reset, moderation,
  authorization role, or multi-factor authentication.
- Direct messages are stored in MySQL without application-level encryption.
- Database availability tests require a separately configured local database and
  are not part of the default isolated test suite.

Use a unique development password for TermLink. Do not reuse an important
password from another service.

## Possible future improvements

- Add TLS or a local secure transport and hide password input.
- Add rate limiting, temporary lockouts, password changes, and account recovery.
- Support multiple rooms with membership and moderation controls.
- Retrieve private-message history with pagination.
- Add reconnect support and retry transient database failures.
- Add configurable retention and encrypted message storage.
- Build a richer terminal UI with scrolling, colors, and input history.
- Add disposable-MySQL integration tests in CI.

## Project layout

```text
TermLink/
├── Cargo.toml
├── Cargo.lock
├── .env.example
├── README.md
├── migrations/
│   └── 202609140001_create_users_and_messages.sql
├── src/
│   ├── bin/
│   │   ├── server.rs
│   │   └── client.rs
│   ├── auth.rs
│   ├── commands.rs
│   ├── database.rs
│   ├── error.rs
│   ├── lib.rs
│   └── protocol.rs
└── tests/
    ├── commands.rs
    ├── protocol.rs
    ├── scaffold.rs
    ├── tcp_protocol.rs
    └── validation.rs
```
