//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

pub mod base;
pub mod item;

use crate::config::DBConfig;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

pub struct DBPool {
    pub sqlite_pool: SqlitePool,
}

impl DBPool {
    pub async fn init(config: &DBConfig) -> anyhow::Result<Self> {
        let sqlite_pool = SqlitePoolOptions::new().connect(&config.sqlite.db_path).await?;

        Ok(Self {
            sqlite_pool,
        })
    }
}
