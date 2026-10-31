//! User-requested metadata only. Cache before network; manual entry stays available.

mod googlebooks;
mod isbnsearch;
pub mod libris;
mod openlibrary;

use std::{future::Future, sync::OnceLock, time::Duration};

use crate::{
    error::{AppError, AppResult},
    isbn,
    models::LookupIsbnResult,
    repositories::settings,
    sqlite_db::{self as sqlx, SqlitePool},
};

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const CACHE_PREFIX: &str = "isbn:";
const SETTING_ENABLED: &str = "metadata.lookup_enabled";

fn allowed_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && matches!(
            url.host_str(),
            Some("openlibrary.org" | "www.googleapis.com" | "www.libris.ro" | "isbnsearch.org")
        )
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!(
                "ShelfLedger/",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/dkvlabs/ShelfLedger)"
            ))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 3
                    || !allowed_url(attempt.url())
                    || attempt.previous().first().map(|u| u.host_str())
                        != Some(attempt.url().host_str())
                {
                    attempt.error("metadata redirect outside provider")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .expect("metadata client")
    })
}

async fn fetch_text(url: &str, provider: &str) -> AppResult<Option<String>> {
    let url = reqwest::Url::parse(url)
        .map_err(|_| AppError::Validation("invalid metadata URL".into()))?;
    if !allowed_url(&url) {
        return Err(AppError::Validation("unsupported metadata URL".into()));
    }
    let mut response = client()
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("{provider} lookup failed: {e}")))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    response = response
        .error_for_status()
        .map_err(|e| AppError::Network(format!("{provider} lookup failed: {e}")))?;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AppError::Network(format!(
            "{provider} response is too large"
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| AppError::Network(format!("{provider} response failed: {e}")))?
    {
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(AppError::Network(format!(
                "{provider} response is too large"
            )));
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| AppError::Network(format!("{provider} response is not UTF-8")))
}

pub(crate) async fn require_online(pool: &SqlitePool) -> AppResult<()> {
    // Explicit metadata actions share the same preference on every platform.
    // Local caches are read before this gate and remain usable offline.
    if settings::get_json(pool, SETTING_ENABLED)
        .await?
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
    {
        Ok(())
    } else {
        Err(AppError::MetadataDisabled)
    }
}

async fn cache_get(pool: &SqlitePool, key: &str) -> AppResult<Option<LookupIsbnResult>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT payload_json FROM metadata_cache WHERE cache_key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(row
        .and_then(|(json,)| serde_json::from_str::<LookupIsbnResult>(&json).ok())
        .map(|mut r| {
            r.cached = true;
            r.published_at = publication_date(r.published_at.as_deref());
            r.page_count = r.page_count.filter(|n| (1..=1_000_000).contains(n));
            r
        }))
}

async fn cache_put(pool: &SqlitePool, key: &str, result: &LookupIsbnResult) -> AppResult<()> {
    sqlx::query("INSERT INTO metadata_cache (cache_key, payload_json, fetched_at) VALUES (?1, ?2, ?3) \
        ON CONFLICT (cache_key) DO UPDATE SET payload_json = excluded.payload_json, fetched_at = excluded.fetched_at")
        .bind(key).bind(serde_json::to_string(result).expect("lookup result"))
        .bind(crate::ids::now_iso()).execute(pool).await?;
    Ok(())
}

fn parse_requested(raw: &str) -> AppResult<isbn::NormalizedIsbn> {
    isbn::parse(raw).ok_or_else(|| AppError::Validation(format!("not a valid ISBN: {raw:?}")))
}

fn publication_date(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if crate::models::validate_publication(Some(raw), None).is_ok() {
        return Some(raw.to_string());
    }
    // Open Library also uses English calendar dates. Never invent day/month precision.
    for format in ["%B %d, %Y", "%b %d, %Y", "%d %B %Y"] {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(raw, format) {
            return Some(date.format("%Y-%m-%d").to_string());
        }
    }
    None
}

pub async fn lookup_isbn(pool: &SqlitePool, raw: &str) -> AppResult<Option<LookupIsbnResult>> {
    let parsed = parse_requested(raw)?;
    let requested = parsed.isbn13.as_deref().expect("ISBN-13 conversion");
    lookup_with_providers(
        pool,
        requested,
        provider_with_timeout(
            "Open Library",
            openlibrary::fetch(requested),
            Duration::from_secs(12),
        ),
        first_match(
            provider_with_timeout(
                "Google Books",
                googlebooks::fetch(requested),
                Duration::from_secs(12),
            ),
            provider_with_timeout(
                "ISBN Search",
                isbnsearch::fetch(requested),
                Duration::from_secs(12),
            ),
        ),
    )
    .await
}

// Bound the whole provider operation, including any secondary requests. A slow
// provider must not prevent an alternative catalog from being tried.
async fn provider_with_timeout(
    name: &str,
    request: impl Future<Output = AppResult<Option<LookupIsbnResult>>>,
    budget: Duration,
) -> AppResult<Option<LookupIsbnResult>> {
    tokio::time::timeout(budget, request)
        .await
        .map_err(|_| AppError::Network(format!("{name} lookup timed out")))?
}

async fn first_match(
    primary: impl Future<Output = AppResult<Option<LookupIsbnResult>>>,
    secondary: impl Future<Output = AppResult<Option<LookupIsbnResult>>>,
) -> AppResult<Option<LookupIsbnResult>> {
    match primary.await {
        matched @ Ok(Some(_)) => matched,
        first => match secondary.await {
            // An outage is not a definitive catalog miss.
            Ok(None) => first,
            result => result,
        },
    }
}

async fn lookup_with_providers(
    pool: &SqlitePool,
    requested: &str,
    primary: impl Future<Output = AppResult<Option<LookupIsbnResult>>>,
    secondary: impl Future<Output = AppResult<Option<LookupIsbnResult>>>,
) -> AppResult<Option<LookupIsbnResult>> {
    let key = format!("{CACHE_PREFIX}{requested}");
    // Preserve caches from versions which only supported Open Library.
    for cache_key in [&key, &format!("openlibrary:isbn:{requested}")] {
        if let Some(cached) = cache_get(pool, cache_key).await? {
            let same_edition = cached
                .isbn13
                .as_deref()
                .or(cached.isbn10.as_deref())
                .and_then(isbn::parse)
                .and_then(|id| id.isbn13)
                .as_deref()
                == Some(requested);
            if same_edition {
                let parsed = parse_requested(requested)?;
                return Ok(Some(LookupIsbnResult {
                    isbn13: parsed.isbn13,
                    isbn10: parsed.isbn10,
                    ..cached
                }));
            }
        }
    }
    require_online(pool).await?;
    let result = first_match(primary, secondary).await?;
    if let Some(result) = &result {
        cache_put(pool, &key, result).await?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn online_metadata_is_available_by_default_and_obeys_the_device_preference() {
        let pool = crate::db::connect_memory().await.unwrap();
        require_online(&pool).await.unwrap();
        settings::set_json(&pool, SETTING_ENABLED, &serde_json::json!(false))
            .await
            .unwrap();
        assert!(matches!(
            require_online(&pool).await,
            Err(AppError::MetadataDisabled)
        ));
        settings::set_json(&pool, SETTING_ENABLED, &serde_json::json!(true))
            .await
            .unwrap();
        require_online(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn uncached_lookup_checks_all_three_sources_after_misses_errors_or_timeouts() {
        use std::sync::Mutex;
        for failure in ["miss", "error", "timeout"] {
            let pool = crate::db::connect_memory().await.unwrap();
            let checked = Mutex::new(Vec::new());
            let lookup = |source: &'static str| {
                let checked = &checked;
                async move {
                    checked.lock().unwrap().push(source);
                    match failure {
                        "miss" => Ok(None),
                        "error" => Err(AppError::Network(format!("{source} unavailable"))),
                        _ => {
                            provider_with_timeout(
                                source,
                                std::future::pending(),
                                Duration::from_millis(1),
                            )
                            .await
                        }
                    }
                }
            };
            let mut expected = book();
            expected.source = "isbnsearch".into();
            let result = lookup_with_providers(
                &pool,
                "9786067882629",
                lookup("openlibrary"),
                first_match(lookup("googlebooks"), async {
                    checked.lock().unwrap().push("isbnsearch");
                    Ok(Some(expected.clone()))
                }),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
            assert_eq!(
                *checked.lock().unwrap(),
                ["openlibrary", "googlebooks", "isbnsearch"]
            );
            settings::set_json(&pool, SETTING_ENABLED, &serde_json::json!(false))
                .await
                .unwrap();
            let cached = lookup_isbn(&pool, "6067882620").await.unwrap().unwrap();
            assert!(cached.cached);
            assert_eq!(cached.source, "isbnsearch");
        }
    }

    #[tokio::test]
    async fn third_provider_handles_misses_errors_and_caches_offline() {
        for primary in [Ok(None), Err(AppError::Network("timeout".into()))] {
            let pool = crate::db::connect_memory().await.unwrap();
            let mut alternative = book();
            alternative.source = "isbnsearch".into();
            let found = lookup_with_providers(
                &pool,
                "9786067882629",
                async { primary },
                first_match(async { Err(AppError::Network("HTTP 429".into())) }, async {
                    Ok(Some(alternative))
                }),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(found.source, "isbnsearch");
            settings::set_json(&pool, SETTING_ENABLED, &serde_json::json!(false))
                .await
                .unwrap();
            let cached = lookup_isbn(&pool, "6067882620").await.unwrap().unwrap();
            assert!(cached.cached);
            assert_eq!(cached.source, "isbnsearch");
        }
    }

    #[tokio::test]
    async fn slow_provider_yields_to_fallback_and_matches_skip_later_providers() {
        let found = first_match(
            provider_with_timeout("slow", std::future::pending(), Duration::from_millis(1)),
            async { Ok(Some(book())) },
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(found.isbn13.as_deref(), Some("9786067882629"));
        assert!(first_match(async { Ok(Some(book())) }, async {
            panic!("unnecessary request")
        })
        .await
        .unwrap()
        .is_some());
        let missing = first_match(
            async { Ok(None) },
            first_match(async { Ok(None) }, async { Ok(None) }),
        )
        .await
        .unwrap();
        assert!(missing.is_none());
        assert!(first_match(
            async { Err(AppError::Network("quota".into())) },
            first_match(async { Ok(None) }, async { Ok(None) })
        )
        .await
        .is_err());
    }

    fn book() -> LookupIsbnResult {
        serde_json::from_value(
            serde_json::json!({"source":"googlebooks", "isbn13":"9786067882629",
            "isbn10":"6067882620", "title":"Mitologie nordica", "subtitle":null,
            "publisher":null,"publishedAt":"2017","language":"ro","pageCount":250,
            "authorNames":["Neil Gaiman"],"cached":false}),
        )
        .unwrap()
    }
    #[tokio::test]
    async fn fallback_after_miss_or_error_and_then_offline_cache() {
        for primary in [
            Ok(None),
            Err(AppError::Network("provider unavailable".into())),
        ] {
            let pool = crate::db::connect_memory().await.unwrap();
            let result = lookup_with_providers(&pool, "9786067882629", async { primary }, async {
                Ok(Some(book()))
            })
            .await
            .unwrap()
            .unwrap();
            assert_eq!(result.source, "googlebooks");
            settings::set_json(&pool, SETTING_ENABLED, &serde_json::json!(false))
                .await
                .unwrap();
            let result =
                lookup_with_providers(&pool, "9786067882629", async { panic!("network") }, async {
                    panic!("network")
                })
                .await
                .unwrap()
                .unwrap();
            assert!(result.cached);
        }
    }
    #[tokio::test]
    async fn disabled_and_legacy_cache_never_poll_providers() {
        let pool = crate::db::connect_memory().await.unwrap();
        settings::set_json(&pool, SETTING_ENABLED, &serde_json::json!(false))
            .await
            .unwrap();
        let error =
            lookup_with_providers(&pool, "9786067882629", async { panic!("network") }, async {
                panic!("network")
            })
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::MetadataDisabled));
        assert_eq!(
            serde_json::to_value(error).unwrap()["kind"],
            "metadata_disabled"
        );
        cache_put(&pool, "openlibrary:isbn:9786067882629", &book())
            .await
            .unwrap();
        assert!(
            lookup_isbn(&pool, "978-606-788-262-9")
                .await
                .unwrap()
                .unwrap()
                .cached
        );
    }
    #[tokio::test]
    async fn legacy_cache_dates_are_saveable_and_wrong_editions_are_ignored() {
        let pool = crate::db::connect_memory().await.unwrap();
        settings::set_json(&pool, SETTING_ENABLED, &serde_json::json!(false))
            .await
            .unwrap();
        let mut result = book();
        result.published_at = Some("May 5, 2017".into());
        cache_put(&pool, "openlibrary:isbn:9786067882629", &result)
            .await
            .unwrap();
        assert_eq!(
            lookup_isbn(&pool, "9786067882629")
                .await
                .unwrap()
                .unwrap()
                .published_at
                .as_deref(),
            Some("2017-05-05")
        );
        result.isbn13 = Some("9780306406157".into());
        cache_put(&pool, "openlibrary:isbn:9786067882629", &result)
            .await
            .unwrap();
        assert!(matches!(
            lookup_isbn(&pool, "9786067882629").await,
            Err(AppError::MetadataDisabled)
        ));
    }
    #[tokio::test]
    async fn primary_match_skips_fallback_and_missing_is_distinct_from_failure() {
        let pool = crate::db::connect_memory().await.unwrap();
        assert!(
            lookup_with_providers(&pool, "9786067882629", async { Ok(None) }, async {
                Ok(None)
            })
            .await
            .unwrap()
            .is_none()
        );
        assert!(lookup_with_providers(
            &pool,
            "9786067882629",
            async { Err(AppError::Network("offline".into())) },
            async { Ok(None) }
        )
        .await
        .is_err());
        assert!(
            lookup_with_providers(&pool, "9786067882629", async { Ok(Some(book())) }, async {
                panic!("fallback")
            })
            .await
            .unwrap()
            .is_some()
        );
        assert!(matches!(
            lookup_isbn(&pool, "invalid").await,
            Err(AppError::Validation(_))
        ));
    }
    #[test]
    fn dates_are_saveable_and_redirect_destinations_are_restricted() {
        assert_eq!(
            publication_date(Some("May 5, 2017")).as_deref(),
            Some("2017-05-05")
        );
        assert_eq!(publication_date(Some("2017")).as_deref(), Some("2017"));
        assert!(publication_date(Some("2017-02-31")).is_none());
        assert!(publication_date(Some("unknown")).is_none());
        for url in [
            "http://www.libris.ro/",
            "https://www.libris.ro.evil.test/",
            "https://127.0.0.1/",
            "https://user@www.libris.ro/",
            "https://www.libris.ro:444/",
            "https://isbnsearch.org.evil.test/isbn/9786067882629",
            "http://isbnsearch.org/isbn/9786067882629",
        ] {
            assert!(!allowed_url(&reqwest::Url::parse(url).unwrap()));
        }
    }
}
