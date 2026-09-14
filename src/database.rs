//! MySQL connection pool used by the chat server.

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
}
