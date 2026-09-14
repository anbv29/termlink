//! MySQL connection pool used by the chat server.

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

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
}
