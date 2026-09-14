//! MySQL connection pool used by the chat server.

use crate::protocol::{GENERAL_ROOM, HistoryMessage};
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::Row;
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: u64,
    pub username: String,
}

pub struct UserCredentials {
    pub user: User,
    pub password_hash: String,
}

#[derive(Clone)]
pub struct Database {
    pool: MySqlPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = MySqlPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        sqlx::migrate!("./migrations").run(&self.pool).await
    }

    pub async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
    ) -> Result<User, sqlx::Error> {
        let result = sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, ?)")
            .bind(username)
            .bind(password_hash)
            .execute(&self.pool)
            .await?;

        Ok(User {
            id: result.last_insert_id(),
            username: username.to_owned(),
        })
    }

    pub fn is_duplicate(error: &sqlx::Error) -> bool {
        matches!(error, sqlx::Error::Database(database) if database.code().as_deref() == Some("1062"))
    }

    pub async fn user_credentials(
        &self,
        username: &str,
    ) -> Result<Option<UserCredentials>, sqlx::Error> {
        let row =
            sqlx::query("SELECT id, username, password_hash FROM users WHERE username = ? LIMIT 1")
                .bind(username)
                .fetch_optional(&self.pool)
                .await?;

        row.map(|row| {
            Ok(UserCredentials {
                user: User {
                    id: row.try_get("id")?,
                    username: row.try_get("username")?,
                },
                password_hash: row.try_get("password_hash")?,
            })
        })
        .transpose()
    }

    pub async fn save_public_message(
        &self,
        sender_id: u64,
        content: &str,
    ) -> Result<DateTime<Utc>, sqlx::Error> {
        let timestamp = Utc::now();
        sqlx::query(
            "INSERT INTO messages (sender_id, recipient_id, room, content, created_at) \
             VALUES (?, NULL, ?, ?, ?)",
        )
        .bind(sender_id)
        .bind(GENERAL_ROOM)
        .bind(content)
        .bind(timestamp.naive_utc())
        .execute(&self.pool)
        .await?;
        Ok(timestamp)
    }

    pub async fn public_history(&self, limit: u32) -> Result<Vec<HistoryMessage>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT users.username, messages.content, messages.created_at \
             FROM messages \
             INNER JOIN users ON users.id = messages.sender_id \
             WHERE messages.room = ? AND messages.recipient_id IS NULL \
             ORDER BY messages.created_at DESC, messages.id DESC \
             LIMIT ?",
        )
        .bind(GENERAL_ROOM)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut messages = rows
            .into_iter()
            .map(|row| {
                let timestamp: NaiveDateTime = row.try_get("created_at")?;
                Ok(HistoryMessage {
                    username: row.try_get("username")?,
                    content: row.try_get("content")?,
                    timestamp: timestamp.and_utc(),
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;
        messages.reverse();
        Ok(messages)
    }

    pub async fn save_direct_message(
        &self,
        sender_id: u64,
        recipient_id: u64,
        content: &str,
    ) -> Result<DateTime<Utc>, sqlx::Error> {
        let timestamp = Utc::now();
        sqlx::query(
            "INSERT INTO messages (sender_id, recipient_id, room, content, created_at) \
             VALUES (?, ?, NULL, ?, ?)",
        )
        .bind(sender_id)
        .bind(recipient_id)
        .bind(content)
        .bind(timestamp.naive_utc())
        .execute(&self.pool)
        .await?;
        Ok(timestamp)
    }
}
