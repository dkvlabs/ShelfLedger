use serde_json::Value;

use super::{fetch_text, publication_date};
use crate::{
    error::{AppError, AppResult},
    isbn,
    models::LookupIsbnResult,
};

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn parse_response(body: &Value, requested: &str) -> Option<LookupIsbnResult> {
    body.get("items")?.as_array()?.iter().find_map(|item| {
        let info = item.get("volumeInfo")?;
        // Search can return related books. Accept only an exact, checksum-valid edition.
        let matched = info
            .get("industryIdentifiers")?
            .as_array()?
            .iter()
            .filter_map(|id| id.get("identifier")?.as_str())
            .any(|raw| isbn::parse(raw).and_then(|isbn| isbn.isbn13).as_deref() == Some(requested));
        if !matched {
            return None;
        }
        let isbn = isbn::parse(requested)?;
        Some(LookupIsbnResult {
            source: "googlebooks".into(),
            isbn13: isbn.isbn13,
            isbn10: isbn.isbn10,
            title: Some(text(info, "title")?),
            subtitle: text(info, "subtitle"),
            publisher: text(info, "publisher"),
            published_at: publication_date(info.get("publishedDate").and_then(Value::as_str)),
            language: text(info, "language"),
            page_count: info
                .get("pageCount")
                .and_then(Value::as_i64)
                .filter(|n| (1..=1_000_000).contains(n)),
            author_names: info
                .get("authors")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            cached: false,
        })
    })
}

pub(super) async fn fetch(requested: &str) -> AppResult<Option<LookupIsbnResult>> {
    let Some(text) = fetch_text(
        &format!("https://www.googleapis.com/books/v1/volumes?q=isbn:{requested}&maxResults=5"),
        "Google Books",
    )
    .await?
    else {
        return Ok(None);
    };
    let body = serde_json::from_str(&text)
        .map_err(|e| AppError::Network(format!("could not parse Google Books response: {e}")))?;
    Ok(parse_response(&body, requested))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_isbn_only_not_first_search_result() {
        let info = serde_json::json!({"title":"Mitologie nordica","authors":["Neil Gaiman"],
            "industryIdentifiers":[{"type":"ISBN_13","identifier":"978-606-788-262-9"}],
            "language":"ro","publisher":"ART","publishedDate":"2017","pageCount":250});
        let mut wrong = info.clone();
        wrong["industryIdentifiers"][0]["identifier"] = "9780306406157".into();
        let body = serde_json::json!({"items":[{"volumeInfo":wrong},{"volumeInfo":info}]});
        let result = parse_response(&body, "9786067882629").unwrap();
        assert_eq!(result.title.as_deref(), Some("Mitologie nordica"));
        assert_eq!(result.author_names, vec!["Neil Gaiman"]);
        assert_eq!(result.published_at.as_deref(), Some("2017"));
        assert!(parse_response(
            &serde_json::json!({"items":[{"volumeInfo":wrong}]}),
            "9786067882629"
        )
        .is_none());
        assert!(parse_response(&serde_json::json!({"totalItems":0}), "9786067882629").is_none());
    }
}
