//! Exact ISBN fallback using the public ISBN Search book page. No search snippets,
//! ads, scripts, images, prices or related-book metadata are imported.
use scraper::{Html, Selector};

use super::{fetch_text, parse_requested, publication_date};
use crate::{
    error::{AppError, AppResult},
    isbn,
    models::LookupIsbnResult,
};

fn parse_html(html: &str, requested: &str) -> AppResult<Option<LookupIsbnResult>> {
    let document = Html::parse_document(html);
    let book_selector = Selector::parse("#book .bookinfo").expect("static selector");
    let title_selector = Selector::parse("h1").expect("static selector");
    let field_selector = Selector::parse("p").expect("static selector");
    let label_selector = Selector::parse("strong").expect("static selector");
    let Some(book) = document.select(&book_selector).next() else {
        // A 200 challenge/error/redesigned page must not look like a catalog miss.
        return Err(AppError::Network(
            "ISBN Search returned an unrecognized book page".into(),
        ));
    };
    let fields: Vec<(String, String)> = book
        .select(&field_selector)
        .filter_map(|p| {
            let label = p.select(&label_selector).next()?.text().collect::<String>();
            let text = p.text().collect::<String>();
            let value = text.trim().strip_prefix(label.trim())?.trim();
            Some((
                label.trim().trim_end_matches(':').to_string(),
                value.to_string(),
            ))
        })
        .collect();
    let field = |label: &str| {
        fields
            .iter()
            .find(|(key, value)| key == label && !value.is_empty())
            .map(|(_, value)| value.clone())
    };
    let matches = [field("ISBN-13"), field("ISBN-10")]
        .into_iter()
        .flatten()
        .any(|raw| isbn::parse(&raw).and_then(|id| id.isbn13).as_deref() == Some(requested));
    if !matches {
        return Ok(None);
    }
    let title = book
        .select(&title_selector)
        .next()
        .map(|h| h.text().collect::<String>().trim().to_string())
        .filter(|title| !title.is_empty())
        .ok_or_else(|| AppError::Network("ISBN Search returned no book title".into()))?;
    let parsed = parse_requested(requested)?;
    Ok(Some(LookupIsbnResult {
        source: "isbnsearch".into(),
        isbn13: parsed.isbn13,
        isbn10: parsed.isbn10,
        title: Some(title),
        subtitle: None,
        publisher: field("Publisher"),
        published_at: publication_date(field("Published").as_deref()),
        language: None,
        page_count: None,
        // Preserve surname-first commas. Only explicit semicolons separate names.
        author_names: field("Author")
            .or_else(|| field("Authors"))
            .into_iter()
            .flat_map(|names| {
                names
                    .split(';')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect(),
        cached: false,
    }))
}

pub(super) async fn fetch(requested: &str) -> AppResult<Option<LookupIsbnResult>> {
    let Some(html) = fetch_text(
        &format!("https://isbnsearch.org/isbn/{requested}"),
        "ISBN Search",
    )
    .await?
    else {
        return Ok(None);
    };
    parse_html(&html, requested)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Only factual fields from the public page fetched on 29.10.2026. Keep the
    // provider's spelling; users review/correct metadata before saving.
    const HTML: &str = r#"<div id="book"><div class="bookinfo">
      <h1>Mitologie Nordiča</h1>
      <p><strong>ISBN-13:</strong> <a>9786067882629</a></p>
      <p><strong>ISBN-10:</strong> <a>6067882620</a></p>
      <p><strong>Author:</strong> Neil Gaiman</p>
      <p><strong>Publisher:</strong> Arthur</p>
      <p><strong>Published:</strong> 2017</p>
    </div></div>"#;

    #[test]
    fn exact_romanian_edition_without_inventing_missing_fields() {
        let book = parse_html(HTML, "9786067882629").unwrap().unwrap();
        assert_eq!(book.title.as_deref(), Some("Mitologie Nordiča"));
        assert_eq!(book.author_names, vec!["Neil Gaiman"]);
        assert_eq!(book.publisher.as_deref(), Some("Arthur"));
        assert_eq!(book.published_at.as_deref(), Some("2017"));
        assert_eq!(book.isbn10.as_deref(), Some("6067882620"));
        assert!(book.page_count.is_none());
        assert!(book.language.is_none());
        assert!(parse_html(HTML, "9780306406157").unwrap().is_none());
        assert!(parse_html(
            &HTML
                .replace("9786067882629", "9786067882628")
                .replace("6067882620", "6067882621"),
            "9786067882629"
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn accepts_isbn10_equivalent_and_preserves_author_commas() {
        let html = HTML
            .replace("9786067882629", "unavailable")
            .replace("Neil Gaiman", "Gaiman, Neil; Pratchett, Terry")
            .replace("2017", "unknown");
        let book = parse_html(&html, "9786067882629").unwrap().unwrap();
        assert_eq!(book.author_names, vec!["Gaiman, Neil", "Pratchett, Terry"]);
        assert!(book.published_at.is_none());
    }

    #[test]
    fn ignores_isbn_outside_book_and_reports_changed_or_challenge_pages() {
        let html = format!("<aside>{HTML}</aside><div id='book'><div class='bookinfo'><h1>Other book</h1></div></div>");
        // An ISBN elsewhere in the document is never enough to validate a book.
        let html = html.replacen("id=\"book\"", "id=\"recommendations\"", 1);
        assert!(parse_html(&html, "9786067882629").unwrap().is_none());
        assert!(parse_html("<h1>Please verify you are human</h1>", "9786067882629").is_err());
        assert!(parse_html(
            &HTML.replace("<h1>Mitologie Nordiča</h1>", ""),
            "9786067882629"
        )
        .is_err());
    }

    #[tokio::test]
    #[ignore = "live provider check; needs network and is not part of deterministic CI"]
    async fn live_isbn_fallback_and_offline_cache() {
        let pool = crate::db::connect_memory().await.unwrap();
        let result = super::super::lookup_isbn(&pool, "9786067882629")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.isbn13.as_deref(), Some("9786067882629"));
        assert!(!result.title.as_deref().unwrap().is_empty());
        assert!(result
            .author_names
            .iter()
            .any(|name| name.contains("Gaiman")));
        super::super::settings::set_json(
            &pool,
            super::super::SETTING_ENABLED,
            &serde_json::json!(false),
        )
        .await
        .unwrap();
        let cached = super::super::lookup_isbn(&pool, "6067882620")
            .await
            .unwrap()
            .unwrap();
        assert!(cached.cached);
        assert_eq!(cached.source, result.source);
    }
}
