//! Open Library metadata lookup (docs §10.1).
//!
//! User-initiated only, identified with a proper User-Agent, cached in the
//! local `metadata_cache` table, and fully optional: when the provider is
//! disabled or the network fails, the UI falls back to manual entry. We never
//! fabricate values for fields the provider did not return.

use crate::sqlite_db as sqlx;
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::isbn;
use crate::models::LookupIsbnResult;
use crate::repositories::settings;

const BASE_URL: &str = "https://openlibrary.org";
/// Contact per Open Library usage guidelines (§12/§10 of the plan).
const USER_AGENT: &str = concat!(
    "ShelfLedger/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/shelfledger/shelfledger)"
);
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);
const SETTING_ENABLED: &str = "metadata.lookup_enabled";
const CACHE_PREFIX: &str = "openlibrary:isbn:";

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(LOOKUP_TIMEOUT)
            .build()
            .expect("reqwest client")
    })
}

async fn is_provider_enabled(pool: &SqlitePool) -> bool {
    // The Android distribution is fully offline, including after restoring
    // desktop preferences. Imported metadata caches remain usable.
    if cfg!(target_os = "android") {
        return false;
    }
    settings::get_json(pool, SETTING_ENABLED)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

fn first_str(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Array(items)) => items
            .iter()
            .find_map(|item| item.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// Parsed response body plus the author keys that still need name resolution.
struct ParsedEdition {
    result: LookupIsbnResult,
    author_keys: Vec<String>,
}

fn parse_isbn_response(body: &Value) -> Option<ParsedEdition> {
    // Missing "title" means the ISBN resolved to nothing useful.
    let title = first_str(body, "title")?;
    let publisher = body
        .get("publishers")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let author_keys: Vec<String> = body
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|a| a.get("key").and_then(|k| k.as_str()))
                .map(|s| s.trim_start_matches("/authors/").to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut page_count = body.get("number_of_pages").and_then(|v| v.as_i64());
    if page_count.is_none() {
        page_count = body
            .get("pagination")
            .and_then(|v| v.as_str())
            .and_then(|s| s.split_whitespace().next())
            .and_then(|t| t.parse::<i64>().ok());
    }
    let page_count = page_count.filter(|p| *p > 0);

    Some(ParsedEdition {
        result: LookupIsbnResult {
            source: "openlibrary".into(),
            isbn13: body
                .get("isbn_13")
                .and_then(|v| v.as_array())
                .and_then(|items| items.first())
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string()),
            isbn10: body
                .get("isbn_10")
                .and_then(|v| v.as_array())
                .and_then(|items| items.first())
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string()),
            title: Some(title),
            subtitle: first_str(body, "subtitle"),
            publisher,
            published_at: first_str(body, "publish_date"),
            language: None,
            page_count,
            author_names: Vec::new(),
            cached: false,
        },
        author_keys,
    })
}

async fn fetch_author_names(keys: &[String]) -> Vec<String> {
    let mut names = Vec::new();
    for key in keys.iter().take(3) {
        let url = format!("{BASE_URL}/authors/{key}.json");
        match client().get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.json::<Value>().await {
                    if let Some(name) = first_str(&body, "name") {
                        names.push(name);
                    }
                }
            }
            _ => break, // provider hiccup: keep what we have
        }
    }
    names
}

async fn cache_get(pool: &SqlitePool, key: &str) -> AppResult<Option<LookupIsbnResult>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT payload_json FROM metadata_cache WHERE cache_key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(json,)| serde_json::from_str(&json).ok()))
}

async fn cache_put(pool: &SqlitePool, key: &str, result: &LookupIsbnResult) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO metadata_cache (cache_key, payload_json, fetched_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT (cache_key) DO UPDATE SET payload_json = excluded.payload_json, \
         fetched_at = excluded.fetched_at",
    )
    .bind(key)
    .bind(serde_json::to_string(result).expect("serializable lookup result"))
    .bind(crate::ids::now_iso())
    .execute(pool)
    .await?;
    Ok(())
}

/// Look up an ISBN. Cache first; then Open Library. `Ok(None)` means "provider
/// found nothing"; a network failure surfaces as `AppError::Network` so the UI
/// can offer manual entry.
pub async fn lookup_isbn(pool: &SqlitePool, raw: &str) -> AppResult<Option<LookupIsbnResult>> {
    let Some(parsed) = isbn::parse(raw) else {
        return Err(AppError::Validation(format!("not a valid ISBN: {raw:?}")));
    };
    let cache_key = format!(
        "{CACHE_PREFIX}{}",
        parsed
            .isbn13
            .clone()
            .or(parsed.isbn10.clone())
            .expect("parse yields at least one form")
    );

    if let Some(mut cached) = cache_get(pool, &cache_key).await? {
        cached.cached = true;
        return Ok(Some(cached));
    }

    if !is_provider_enabled(pool).await {
        return Ok(None);
    }

    let requested = parsed
        .isbn13
        .as_deref()
        .or(parsed.isbn10.as_deref())
        .expect("parse yields at least one form");
    let url = format!("{BASE_URL}/isbn/{requested}.json");
    let response = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Open Library lookup failed: {e}")))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response = response
        .error_for_status()
        .map_err(|e| AppError::Network(format!("Open Library returned an error: {e}")))?;
    let body: Value = response
        .json()
        .await
        .map_err(|e| AppError::Network(format!("could not parse Open Library response: {e}")))?;

    let Some(body_parsed) = parse_isbn_response(&body) else {
        return Ok(None);
    };
    let mut result = body_parsed.result;
    result.author_names = fetch_author_names(&body_parsed.author_keys).await;
    result.isbn13 = result.isbn13.or(parsed.isbn13.clone());
    result.isbn10 = result.isbn10.or(parsed.isbn10.clone());

    cache_put(pool, &cache_key, &result).await?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_openlibrary_body() {
        let body: Value = serde_json::json!({
            "title": "Blink",
            "subtitle": "The Power of Thinking Without Thinking",
            "publishers": ["Little, Brown and Company"],
            "publish_date": "2005",
            "number_of_pages": 277,
            "isbn_13": ["9780316010665"],
            "isbn_10": ["0316010669"],
            "authors": [{"key": "/authors/OL1397136A"}]
        });
        let parsed = parse_isbn_response(&body).expect("parsed");
        assert_eq!(parsed.result.title.as_deref(), Some("Blink"));
        assert_eq!(
            parsed.result.subtitle.as_deref(),
            Some("The Power of Thinking Without Thinking")
        );
        assert_eq!(
            parsed.result.publisher.as_deref(),
            Some("Little, Brown and Company")
        );
        assert_eq!(parsed.result.page_count, Some(277));
        assert_eq!(parsed.author_keys, vec!["OL1397136A"]);
        assert!(!parsed.result.cached);
    }

    #[test]
    fn parses_pagination_fallback_when_page_count_missing() {
        let body: Value = serde_json::json!({
            "title": "X",
            "pagination": "304 pages ;",
            "authors": []
        });
        let parsed = parse_isbn_response(&body).expect("parsed");
        assert_eq!(parsed.result.page_count, Some(304));
    }

    #[test]
    fn body_without_title_is_not_a_match() {
        let body: Value = serde_json::json!({ "publishers": ["X"] });
        assert!(parse_isbn_response(&body).is_none());
    }

    #[tokio::test]
    async fn lookup_rejects_invalid_isbn_without_network() {
        let pool = crate::db::connect_memory().await.unwrap();
        let err = lookup_isbn(&pool, "not-an-isbn").await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn disabled_provider_short_circuits_before_network() {
        let pool = crate::db::connect_memory().await.unwrap();
        settings::set_json(&pool, SETTING_ENABLED, &serde_json::json!(false))
            .await
            .unwrap();
        // Valid ISBN, provider off -> Ok(None), no network access attempted.
        let result = lookup_isbn(&pool, "9780306406157").await.unwrap();
        assert!(result.is_none());
    }
}
