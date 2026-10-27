//! SQLite connection setup: WAL journaling, foreign keys ON, migrations.

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};
use std::str::FromStr;

use crate::error::AppResult;

pub(crate) const MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

async fn connect_opts(url: &str) -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// Open (or create) the library database at `path` and run migrations.
pub async fn connect(path: &std::path::Path) -> AppResult<SqlitePool> {
    let url = format!("sqlite:{}", path.to_string_lossy());
    connect_opts(&url).await
}

/// In-memory database for tests: same pragmas and migrations.
#[allow(dead_code)] // used by #[cfg(test)] modules across the crate
pub async fn connect_memory() -> AppResult<SqlitePool> {
    connect_opts("sqlite::memory:").await
}
