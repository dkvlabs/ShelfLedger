//! SQLite connection setup: WAL journaling, foreign keys ON, migrations.

use crate::sqlite_db as sqlx;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};
use std::str::FromStr;

use crate::error::AppResult;

pub(crate) static MIGRATOR: std::sync::LazyLock<sqlx::migrate::Migrator> =
    std::sync::LazyLock::new(|| {
        use sqlx::migrate::{Migration, MigrationType, Migrator};
        let migrations = [
            (1, "init", include_str!("../migrations/0001_init.sql")),
            (
                2,
                "reading loans finance",
                include_str!("../migrations/0002_reading_loans_finance.sql"),
            ),
            (
                3,
                "fts coverage attachments",
                include_str!("../migrations/0003_fts_coverage_attachments.sql"),
            ),
            (
                4,
                "listing invariants",
                include_str!("../migrations/0004_listing_invariants.sql"),
            ),
            (
                5,
                "ISO currency scales",
                include_str!("../migrations/0005_iso_currency_scales.sql"),
            ),
            (
                6,
                "market comparables",
                include_str!("../migrations/0006_market_comparables.sql"),
            ),
        ]
        .into_iter()
        .map(|(version, name, sql)| {
            Migration::new(
                version,
                name.into(),
                MigrationType::Simple,
                sql.into(),
                false,
            )
        })
        .collect::<Vec<_>>();
        Migrator {
            migrations: migrations.into(),
            ..Migrator::DEFAULT
        }
    });

async fn connect_opts(options: SqliteConnectOptions) -> AppResult<SqlitePool> {
    let options = options
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
    connect_opts(SqliteConnectOptions::new().filename(path)).await
}

/// In-memory database for tests: same pragmas and migrations.
#[allow(dead_code)] // used by #[cfg(test)] modules across the crate
pub async fn connect_memory() -> AppResult<SqlitePool> {
    connect_opts(SqliteConnectOptions::from_str("sqlite::memory:")?).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn database_paths_are_literal_filenames_and_survive_canonicalization() {
        let directory = std::env::temp_dir()
            .join(crate::ids::new_id())
            .join("Library %25 # 文 books");
        std::fs::create_dir_all(&directory).unwrap();
        // Windows canonicalization adds a \\?\ prefix; URL parsing also corrupts %25.
        let directory = directory.canonicalize().unwrap();
        let path = directory.join("library.db");
        let pool = connect(&path).await.unwrap();
        sqlx::query("INSERT INTO settings(key,value_json) VALUES('path-fixture','true')")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        assert!(path.is_file());
        let reopened = connect(&path).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value_json FROM settings WHERE key='path-fixture'"
            )
            .fetch_one(&reopened)
            .await
            .unwrap(),
            "true"
        );
        reopened.close().await;
        crate::file_cleanup::remove_dir_all(directory.parent().unwrap())
            .await
            .unwrap();
    }
}
