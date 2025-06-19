use crate::config::DBConfig;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

pub struct DBPool {
    #[cfg(feature = "sqlite")]
    pub sqlite_pool: SqlitePool,
}

impl DBPool {
    pub async fn init(config: &DBConfig) -> anyhow::Result<Self> {
        #[cfg(feature = "sqlite")]
        let sqlite_pool = SqlitePoolOptions::new().connect(&config.sqlite.db_path).await?;

        Ok(Self {
            #[cfg(feature = "sqlite")]
            sqlite_pool,
        })
    }
}
