//! Open Library metadata lookup (docs §10.1).
//!
//! User-initiated only, identified with a proper User-Agent, cached in the
//! local `metadata_cache` table, and fully optional: when the provider is
//! disabled or the network fails, the UI falls back to manual entry. We never
//! fabricate values for fields the provider did not return.

use super::{fetch_text, publication_date};
use crate::{
    error::{AppError, AppResult},
    isbn,
    models::LookupIsbnResult,
};
use serde_json::Value;
const BASE_URL: &str = "https://openlibrary.org";

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
    let page_count = page_count.filter(|p| (1..=1_000_000).contains(p));

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
            published_at: publication_date(body.get("publish_date").and_then(Value::as_str)),
            language: None,
            page_count,
            author_names: Vec::new(),
            cached: false,
        },
        author_keys,
    })
}

async fn fetch_author_names<F, Fut>(
    keys: &[String],
    deadline: tokio::time::Instant,
    fetch: F,
) -> Vec<String>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = AppResult<Option<String>>>,
{
    let mut names = Vec::new();
    for key in keys.iter().take(3) {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        let url = format!("{BASE_URL}/authors/{key}.json");
        if !key.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        if let Ok(Ok(Some(text))) = tokio::time::timeout_at(deadline, fetch(url)).await {
            if let Ok(body) = serde_json::from_str::<Value>(&text) {
                if let Some(name) = first_str(&body, "name") {
                    names.push(name);
                }
            }
        } else {
            break;
        }
    }
    names
}

pub(super) async fn fetch(requested: &str) -> AppResult<Option<LookupIsbnResult>> {
    // Leave time to return an already-matched edition before the 12-second
    // provider deadline, even if optional author enrichment stalls.
    let author_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(11);
    let Some(text) =
        fetch_text(&format!("{BASE_URL}/isbn/{requested}.json"), "Open Library").await?
    else {
        return Ok(None);
    };
    let body: Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Network(format!("could not parse Open Library response: {e}")))?;
    let Some(parsed) = parse_isbn_response(&body) else {
        return Ok(None);
    };
    // If the provider supplies identifiers, they must include the requested edition.
    let identifiers: Vec<&str> = ["isbn_13", "isbn_10"]
        .iter()
        .filter_map(|key| body.get(key).and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !identifiers.is_empty()
        && !identifiers
            .iter()
            .any(|id| isbn::parse(id).and_then(|id| id.isbn13).as_deref() == Some(requested))
    {
        return Ok(None);
    }
    let mut result = parsed.result;
    let isbn = super::parse_requested(requested)?;
    result.isbn13 = isbn.isbn13;
    result.isbn10 = isbn.isbn10;
    result.author_names =
        fetch_author_names(&parsed.author_keys, author_deadline, |url| async move {
            fetch_text(&url, "Open Library").await
        })
        .await;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn optional_author_timeout_preserves_completed_names() {
        let keys = vec!["OL1A".into(), "OL2A".into(), "OL3A".into()];
        let names = fetch_author_names(
            &keys,
            tokio::time::Instant::now() + std::time::Duration::from_millis(10),
            |url| async move {
                if url.ends_with("/OL1A.json") {
                    Ok(Some(r#"{"name":"First author"}"#.into()))
                } else {
                    std::future::pending().await
                }
            },
        )
        .await;
        assert_eq!(names, vec!["First author"]);
    }

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
}
