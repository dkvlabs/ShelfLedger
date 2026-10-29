//! Import factual Book JSON-LD from an explicitly supplied Libris product page.
//! Libris does not consistently index ISBNs in its public search. No guessed matches.
use scraper::{Html, Selector};
use serde_json::Value;

use super::{
    cache_get, cache_put, enabled, fetch_text, parse_requested, publication_date, CACHE_PREFIX,
};
use crate::{
    error::{AppError, AppResult},
    isbn,
    models::LookupIsbnResult,
    sqlite_db::SqlitePool,
};

fn product_url(raw: &str) -> AppResult<String> {
    let mut url = reqwest::Url::parse(raw.trim()).map_err(|_| {
        AppError::Validation("use a Libris book link (https://www.libris.ro/carte/...)".into())
    })?;
    let parts: Vec<_> = url.path_segments().into_iter().flatten().collect();
    if raw.len() > 2048
        || !super::allowed_url(&url)
        || url.host_str() != Some("www.libris.ro")
        || parts.len() != 3
        || parts[0] != "carte"
        || parts[1].is_empty()
        || parts[2].is_empty()
        || !parts[2].chars().all(|c| c.is_ascii_digit())
    {
        return Err(AppError::Validation(
            "use a Libris book link (https://www.libris.ro/carte/...)".into(),
        ));
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

fn named(value: &Value) -> Vec<String> {
    match value {
        Value::String(s) => vec![s.trim().to_string()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect(),
        Value::Array(items) => items.iter().flat_map(named).collect(),
        Value::Object(_) => value.get("name").map(named).unwrap_or_default(),
        _ => vec![],
    }
}

fn parse_book(value: &Value) -> Option<LookupIsbnResult> {
    if let Some(items) = value.as_array() {
        return items.iter().find_map(parse_book);
    }
    if let Some(graph) = value.get("@graph") {
        if let Some(book) = parse_book(graph) {
            return Some(book);
        }
    }
    if !named(value.get("@type")?)
        .iter()
        .any(|t| t == "Book" || t == "https://schema.org/Book")
    {
        return None;
    }
    let isbn = value
        .get("isbn")
        .or_else(|| value.get("gtin13"))?
        .as_str()
        .and_then(isbn::parse)?;
    let authors = value.get("author").map(named).unwrap_or_default();
    let mut title = value.get("name")?.as_str()?.trim().to_string();
    // Libris's structured title includes its author; remove only that exact suffix.
    if let Some(author) = authors.first() {
        if let Some(book_title) = title.strip_suffix(&format!(" - {author}")) {
            title = book_title.trim().to_string();
        }
    }
    if title.is_empty() {
        return None;
    }
    let pages = value
        .get("numberOfPages")
        .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()));
    Some(LookupIsbnResult {
        source: "libris".into(),
        isbn13: isbn.isbn13,
        isbn10: isbn.isbn10,
        title: Some(title),
        subtitle: None,
        publisher: value
            .get("publisher")
            .map(named)
            .and_then(|names| names.into_iter().next()),
        published_at: publication_date(value.get("datePublished").and_then(Value::as_str)),
        language: value
            .get("inLanguage")
            .and_then(Value::as_str)
            .map(str::to_owned),
        page_count: pages.filter(|n| (1..=1_000_000).contains(n)),
        author_names: authors,
        cached: false,
    })
}

fn parse_html(html: &str) -> Option<LookupIsbnResult> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("script[type='application/ld+json']").expect("static selector");
    document.select(&selector).find_map(|script| {
        serde_json::from_str::<Value>(&script.text().collect::<String>())
            .ok()
            .and_then(|v| parse_book(&v))
    })
}

fn check_expected(result: &LookupIsbnResult, expected: Option<&str>) -> AppResult<()> {
    if let Some(expected) = expected {
        let expected = parse_requested(expected)?;
        if expected.isbn13 != result.isbn13 {
            return Err(AppError::Validation(
                "the Libris book does not match the entered ISBN".into(),
            ));
        }
    }
    Ok(())
}

pub async fn lookup(
    pool: &SqlitePool,
    raw_url: &str,
    expected: Option<&str>,
) -> AppResult<Option<LookupIsbnResult>> {
    let url = product_url(raw_url)?;
    if let Some(expected) = expected {
        parse_requested(expected)?;
    }
    let key = format!("libris:url:{url}");
    if let Some(cached) = cache_get(pool, &key).await? {
        check_expected(&cached, expected)?;
        return Ok(Some(cached));
    }
    if !enabled(pool).await {
        return Ok(None);
    }
    let Some(html) = fetch_text(&url, "Libris").await? else {
        return Ok(None);
    };
    let Some(result) = parse_html(&html) else {
        return Ok(None);
    };
    check_expected(&result, expected)?;
    cache_put(pool, &key, &result).await?;
    if let Some(isbn13) = &result.isbn13 {
        cache_put(pool, &format!("{CACHE_PREFIX}{isbn13}"), &result).await?;
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    const URL: &str = "https://www.libris.ro/carte/mitologie-nordica-neil-gaiman/1258850";
    // Factual metadata only, extracted from the user's product page on 29.10.2026.
    const HTML: &str = r#"<script type='application/ld+json'>{"@type":"Organization","name":"Libris"}</script>
        <script type='application/ld+json'>{"@graph":[{"@type":["Product","Book"],
        "name":"Mitologie nordica - Neil Gaiman","isbn":"9786067882629","inLanguage":"ro",
        "publisher":{"name":"GRUPUL EDITORIAL ART"},"datePublished":"2017","numberOfPages":250,
        "author":[{"name":"Neil Gaiman"}]}]}</script>"#;

    #[test]
    fn romanian_book_from_json_ld_and_exact_isbn_validation() {
        let result = parse_html(HTML).unwrap();
        assert_eq!(result.title.as_deref(), Some("Mitologie nordica"));
        assert_eq!(result.author_names, vec!["Neil Gaiman"]);
        assert_eq!(result.isbn13.as_deref(), Some("9786067882629"));
        assert_eq!(result.language.as_deref(), Some("ro"));
        assert_eq!(result.page_count, Some(250));
        assert_eq!(result.published_at.as_deref(), Some("2017"));
        assert!(check_expected(&result, Some("978-606-788-262-9")).is_ok());
        assert!(check_expected(&result, Some("9780306406157")).is_err());
        assert!(parse_html(&HTML.replace("9786067882629", "9786067882628")).is_none());
        assert!(parse_html("<html>not found</html>").is_none());
    }
    #[test]
    fn rejects_arbitrary_hosts_and_non_product_urls() {
        assert_eq!(
            product_url(&format!("{URL}?tracking=1#reviews")).unwrap(),
            URL
        );
        for url in [
            "https://www.libris.ro/search?iv.q=x",
            "https://www.libris.ro.evil.test/carte/a/1",
            "http://www.libris.ro/carte/a/1",
            "https://user@www.libris.ro/carte/a/1",
            "https://127.0.0.1/carte/a/1",
            "https://www.libris.ro/carte/a/%2e%2e",
        ] {
            assert!(product_url(url).is_err(), "{url}");
        }
    }
    #[tokio::test]
    async fn cached_libris_result_can_be_used_by_isbn_offline() {
        let pool = crate::db::connect_memory().await.unwrap();
        crate::repositories::settings::set_json(
            &pool,
            super::super::SETTING_ENABLED,
            &serde_json::json!(false),
        )
        .await
        .unwrap();
        assert!(lookup(&pool, URL, None).await.unwrap().is_none());
        let result = parse_html(HTML).unwrap();
        cache_put(&pool, &format!("libris:url:{URL}"), &result)
            .await
            .unwrap();
        cache_put(&pool, "isbn:9786067882629", &result)
            .await
            .unwrap();
        assert!(
            lookup(&pool, URL, Some("9786067882629"))
                .await
                .unwrap()
                .unwrap()
                .cached
        );
        assert!(lookup(&pool, URL, Some("9780306406157")).await.is_err());
        assert_eq!(
            super::super::lookup_isbn(&pool, "9786067882629")
                .await
                .unwrap()
                .unwrap()
                .source,
            "libris"
        );
    }

    #[tokio::test]
    #[ignore = "live provider check; needs network and is not part of deterministic CI"]
    async fn live_user_supplied_libris_book() {
        let pool = crate::db::connect_memory().await.unwrap();
        let result = lookup(&pool, URL, Some("9786067882629"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.title.as_deref(), Some("Mitologie nordica"));
        assert_eq!(result.author_names, vec!["Neil Gaiman"]);
        assert_eq!(result.page_count, Some(250));
        let cached = super::super::lookup_isbn(&pool, "9786067882629")
            .await
            .unwrap()
            .unwrap();
        assert!(cached.cached);
        // Exercise the actual Add Book validation with live provider metadata.
        let input: crate::models::AddBookInput = serde_json::from_value(serde_json::json!({
            "title": result.title, "authorNames": result.author_names,
            "isbn13": result.isbn13, "isbn10": result.isbn10,
            "publisher": result.publisher, "publishedAt": result.published_at,
            "language": result.language, "pageCount": result.page_count,
            "copy": {"conditionGrade":"good"}
        }))
        .unwrap();
        let created = crate::domain::catalog::create_book(&pool, &input)
            .await
            .unwrap();
        assert!(created.warning.is_none());
    }
}
