//! Key/value local preferences stored as JSON.

use crate::sqlite_db as sqlx;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};

pub async fn get_json(pool: &SqlitePool, key: &str) -> AppResult<Option<Value>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value_json FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|(json,)| serde_json::from_str(&json).ok()))
}

pub async fn set_json(pool: &SqlitePool, key: &str, value: &Value) -> AppResult<()> {
    if key == "preferences" {
        let preferences = value
            .as_object()
            .ok_or_else(|| AppError::Validation("preferences must be an object".into()))?;
        if let Some(code) = preferences.get("currency") {
            let code = code
                .as_str()
                .ok_or_else(|| AppError::Validation("currency must be an ISO code".into()))?;
            crate::money::validate_currency(code).map_err(AppError::Validation)?;
        }
        if let Some(language) = preferences.get("language") {
            let supported: Value =
                serde_json::from_str(include_str!("../../../src/lib/locales/languages.json"))
                    .expect("bundled language registry");
            if language
                .as_str()
                .is_none_or(|code| supported.get(code).is_none())
            {
                return Err(AppError::Validation(
                    "unsupported interface language".into(),
                ));
            }
        }
        if preferences
            .get("theme")
            .is_some_and(|theme| !matches!(theme.as_str(), Some("light" | "dark" | "system")))
        {
            return Err(AppError::Validation("unsupported theme".into()));
        }
    }
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

#[cfg(test)]
mod localization_tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    async fn every_registered_language_and_current_currency_persists() {
        let pool = crate::db::connect_memory().await.unwrap();
        let languages: Value =
            serde_json::from_str(include_str!("../../../src/lib/locales/languages.json")).unwrap();
        for language in languages.as_object().unwrap().keys() {
            let preferences = json!({"language":language,"currency":"CLF","theme":"dark","futurePreference":true});
            set_json(&pool, "preferences", &preferences).await.unwrap();
            assert_eq!(
                get_json(&pool, "preferences").await.unwrap(),
                Some(preferences)
            );
        }
        let currencies: Value =
            serde_json::from_str(include_str!("../../../src/lib/currency-catalog.json")).unwrap();
        for currency in currencies["current"].as_object().unwrap().keys() {
            set_json(
                &pool,
                "preferences",
                &json!({"currency":currency,"language":"ur"}),
            )
            .await
            .unwrap();
        }
        let before = get_json(&pool, "preferences").await.unwrap();
        for invalid in [
            json!({"currency":"XYZ"}),
            json!({"language":"zz"}),
            json!({"language":1}),
            json!({"theme":"invalid"}),
            json!(null),
        ] {
            assert!(set_json(&pool, "preferences", &invalid).await.is_err());
            assert_eq!(get_json(&pool, "preferences").await.unwrap(), before);
        }
    }
}
