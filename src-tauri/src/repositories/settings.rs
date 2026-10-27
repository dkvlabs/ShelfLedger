//! Key/value local preferences stored as JSON.

use crate::sqlite_db as sqlx;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::error::AppResult;

pub async fn get_json(pool: &SqlitePool, key: &str) -> AppResult<Option<Value>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value_json FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|(json,)| serde_json::from_str(&json).ok()))
}

pub async fn set_json(pool: &SqlitePool, key: &str, value: &Value) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO settings (key, value_json) VALUES (?1, ?2) \
         ON CONFLICT (key) DO UPDATE SET value_json = excluded.value_json",
    )
    .bind(key)
    .bind(value.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn all(pool: &SqlitePool) -> AppResult<std::collections::HashMap<String, Value>> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value_json FROM settings")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(k, v)| serde_json::from_str(&v).ok().map(|val| (k, val)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn set_then_get_roundtrip() {
        let pool = crate::db::connect_memory().await.unwrap();
        assert_eq!(get_json(&pool, "currency").await.unwrap(), None);
        set_json(&pool, "currency", &json!({"currency": "RON"}))
            .await
            .unwrap();
        assert_eq!(
            get_json(&pool, "currency").await.unwrap(),
            Some(json!({"currency": "RON"}))
        );
        set_json(&pool, "currency", &json!({"currency": "EUR"}))
            .await
            .unwrap();
        let all = all(&pool).await.unwrap();
        assert_eq!(all.get("currency"), Some(&json!({"currency": "EUR"})));
    }
}
