//! Import (Phase 4): Goodreads CSV with preview + duplicate resolution, and a
//! generic JSON import used for portable exports and the demo dataset.
//! No Goodreads API, no scraping — the user brings their own exported file.

use serde::Deserialize;
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::ids;
use crate::isbn;
use crate::models::{ImportCommitInput, ImportPreview, ImportResult, ImportRowPreview};

fn clean_isbn(raw: &str) -> String {
    // Goodreads exports ISBNs as ="9780306406157"; strip that wrapper.
    let s = raw.trim().trim_start_matches('=');
    let s = s.trim_matches(|c| c == '"');
    isbn::normalize(s)
}

fn parse_row_number_map<'a>(
    headers: &'a [String],
    record: &'a csv::StringRecord,
) -> impl Fn(&str) -> Option<&'a str> + 'a {
    move |name: &str| -> Option<&'a str> {
        headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case(name))
            .and_then(|i| record.get(i))
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

fn parse_goodreads_date(raw: &str) -> Option<String> {
    // Goodreads: "2024/03/05" or "2024/03/00" (unknown day) or ISO already.
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y/%m/%d") {
        return Some(d.to_string());
    }
    if chrono::NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d").is_ok() {
        return Some(s[..10].to_string());
    }
    None
}

/// Parse a Goodreads CSV export into preview rows (no writes).
pub fn preview_goodreads_csv(csv_text: &str) -> AppResult<ImportPreview> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(csv_text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| AppError::Validation(format!("could not read CSV headers: {e}")))?
        .clone();
    let header_names: Vec<String> = headers.iter().map(|h| h.trim().to_string()).collect();

    let mut rows = Vec::new();
    for (i, record) in reader.records().enumerate() {
        let record =
            record.map_err(|e| AppError::Validation(format!("bad CSV row {}: {e}", i + 2)))?;
        let field = parse_row_number_map(&header_names, &record);

        let title = field("Title").unwrap_or("").to_string();
        let author = field("Author").unwrap_or("").to_string();
        let raw_isbn = field("ISBN").or_else(|| field("ISBN13")).unwrap_or("");
        let cleaned = clean_isbn(raw_isbn);
        let parsed_isbn = (!cleaned.is_empty())
            .then(|| isbn::parse(&cleaned))
            .flatten();
        let shelf = field("Bookshelves").map(str::to_string);
        let rating = field("My Rating")
            .and_then(|r| r.parse::<i64>().ok())
            .filter(|r| (0..=5).contains(r));

        let mut warnings = Vec::new();
        if title.is_empty() {
            warnings.push("missing title (row will be skipped)".to_string());
        }
        if raw_isbn.is_empty() {
            warnings.push("no ISBN — imported as a manual edition".to_string());
        } else if parsed_isbn.is_none() {
            warnings.push("ISBN failed checksum — imported without ISBN".to_string());
        }
        if let Some(rating_raw) = field("My Rating") {
            let parsed = rating_raw.parse::<i64>().ok();
            if parsed.is_none() && !rating_raw.is_empty() {
                warnings.push(format!("unparseable rating {rating_raw:?} ignored"));
            }
        }
        let date_read = field("Date Read").and_then(parse_goodreads_date);

        rows.push(ImportRowPreview {
            row_number: i + 1,
            title,
            author,
            isbn13: parsed_isbn.as_ref().and_then(|n| n.isbn13.clone()),
            shelf,
            rating,
            date_read,
            warnings,
            duplicate_of: None,
        });
    }
    Ok(ImportPreview {
        source: "goodreads_csv".to_string(),
        rows,
    })
}

/// Fill `duplicate_of` for a preview by checking the library.
pub async fn annotate_duplicates(
    pool: &SqlitePool,
    mut preview: ImportPreview,
) -> AppResult<ImportPreview> {
    for row in preview.rows.iter_mut() {
        if row.title.trim().is_empty() {
            continue;
        }
        let found = crate::domain::catalog::find_duplicates(
            pool,
            &crate::models::DuplicateQuery {
                isbn: row.isbn13.clone(),
                title: Some(row.title.clone()),
                author_name: (!row.author.is_empty()).then(|| row.author.clone()),
            },
        )
        .await?;
        row.duplicate_of = found
            .first()
            .map(|w| format!("{} ({})", w.title, w.author_names.join(", ")));
    }
    Ok(preview)
}

/// Commit selected preview rows. `csv_path` is re-read by Rust; the webview
/// never touches the filesystem.
pub async fn commit_goodreads_csv(
    pool: &SqlitePool,
    csv_path: &str,
    input: &ImportCommitInput,
) -> AppResult<ImportResult> {
    let csv_text = std::fs::read_to_string(csv_path)
        .map_err(|e| AppError::Validation(format!("could not read import file: {e}")))?;
    let preview = preview_goodreads_csv(&csv_text)?;
    let preview = annotate_duplicates(pool, preview).await?;

    let job_id = ids::new_id();
    sqlx::query("INSERT INTO import_jobs (id, source, started_at, status) VALUES (?1, 'goodreads_csv', ?2, 'running')")
        .bind(&job_id)
        .bind(ids::now_iso())
        .execute(pool)
        .await?;

    let mut result = ImportResult {
        imported_works: 0,
        imported_editions: 0,
        imported_copies: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    for row in &preview.rows {
        if !input.row_numbers.contains(&row.row_number) {
            continue;
        }
        if row
            .warnings
            .iter()
            .any(|w| w.contains("row will be skipped"))
        {
            result.skipped += 1;
            continue;
        }
        match import_one_row(pool, row, input).await {
            Ok((work_created, edition_created, copy_created)) => {
                if work_created {
                    result.imported_works += 1;
                }
                if edition_created {
                    result.imported_editions += 1;
                }
                if copy_created {
                    result.imported_copies += 1;
                }
            }
            Err(e) => {
                result.errors.push(format!("row {}: {}", row.row_number, e));
            }
        }
    }

    // Per-row failures are reported in `errors`, not fatal (docs/import-format.md).
    sqlx::query(
        "UPDATE import_jobs SET finished_at = ?2, status = ?3, stats_json = ?4 WHERE id = ?1",
    )
    .bind(&job_id)
    .bind(ids::now_iso())
    .bind("completed")
    .bind(serde_json::to_string(&result).unwrap_or_default())
    .execute(pool)
    .await?;
    Ok(result)
}

/// Import one preview row -> (work_created, edition_created, copy_created).
/// Creates a reading session from shelf/rating when present; never fabricates
/// dates. Duplicate policy: "skip" drops the row, "add_copy" attaches a copy
/// to the existing edition, "new_work" creates a fresh work.
async fn import_one_row(
    pool: &SqlitePool,
    row: &ImportRowPreview,
    input: &ImportCommitInput,
) -> AppResult<(bool, bool, bool)> {
    // Re-resolve the duplicate at commit time.
    let dup = crate::domain::catalog::find_duplicates(
        pool,
        &crate::models::DuplicateQuery {
            isbn: row.isbn13.clone(),
            title: Some(row.title.clone()),
            author_name: (!row.author.is_empty()).then(|| row.author.clone()),
        },
    )
    .await?;

    let mut work_created = false;
    let mut edition_created = false;
    let mut copy_created = false;

    let (work_id, edition_id) = if let Some(existing) = dup.first() {
        match input.duplicate_policy.as_str() {
            "skip" => return Ok((false, false, false)),
            "add_copy" => {
                // Merge-as-copy: attach to the edition that matches the ISBN,
                // else the work's first edition (import-format.md).
                let editions =
                    crate::repositories::editions::list_editions_for_work(pool, &existing.id)
                        .await?;
                let target = editions
                    .iter()
                    .find(|e| row.isbn13.is_some() && e.isbn13.as_deref() == row.isbn13.as_deref())
                    .or_else(|| editions.first());
                let Some(edition) = target else {
                    return Err(AppError::Validation(
                        "duplicate work has no edition to attach a copy to".into(),
                    ));
                };
                crate::domain::catalog::create_copy(
                    pool,
                    &crate::models::CreateCopyInput {
                        edition_id: edition.id.clone(),
                        inventory_code: None,
                        location_id: None,
                        condition_grade: "good".into(),
                        acquisition_type: None,
                        acquired_at: None,
                        note: None,
                        sentimental_score: None,
                        acquisition: None,
                    },
                )
                .await?;
                copy_created = true;
                (existing.id.clone(), Some(edition.id.clone()))
            }
            "new_work" => {
                let (w, e) = create_work_and_edition(pool, row).await?;
                work_created = true;
                edition_created = !e.is_empty();
                (w, (!e.is_empty()).then_some(e))
            }
            other => {
                return Err(AppError::Validation(format!(
                    "unknown duplicate policy {other:?}"
                )));
            }
        }
    } else {
        let (w, e) = create_work_and_edition(pool, row).await?;
        work_created = true;
        edition_created = !e.is_empty();
        (w, (!e.is_empty()).then_some(e))
    };
    let edition_for_session = edition_id.clone();

    // Reading session from shelf + rating + date read (never invented).
    if let Some(shelf) = &row.shelf {
        let status = match shelf.as_str() {
            "read" => Some("finished"),
            "currently-reading" => Some("reading"),
            "to-read" => Some("want_to_read"),
            _ => None,
        };
        if let Some(status) = status {
            let existing = crate::domain::reading::list_sessions_for_work(pool, &work_id).await?;
            let already = existing.iter().any(|s| s.status == status);
            if !already {
                let session = crate::domain::reading::create_session(
                    pool,
                    &crate::models::CreateReadingSessionInput {
                        work_id: work_id.clone(),
                        edition_id: edition_for_session.clone(),
                        status: status.to_string(),
                        started_at: (status == "reading").then(ids::now_iso),
                    },
                )
                .await?;
                if status == "finished" {
                    crate::domain::reading::update_session(
                        pool,
                        &session.id,
                        &crate::models::UpdateReadingSessionInput {
                            status: None,
                            started_at: None,
                            finished_at: row.date_read.clone(),
                            rating: row.rating,
                            review: None,
                        },
                    )
                    .await?;
                }
            }
        }
    }
    Ok((work_created, edition_created, copy_created))
}

async fn create_work_and_edition(
    pool: &SqlitePool,
    row: &ImportRowPreview,
) -> AppResult<(String, String)> {
    let detail = crate::domain::catalog::create_work(
        pool,
        &crate::models::CreateWorkInput {
            title: row.title.clone(),
            subtitle: None,
            original_language: None,
            description: None,
            author_names: row
                .author
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        },
    )
    .await?;
    // The preview only carries validated ISBN-13s; failed checksums import
    // without an ISBN (manual edition).
    let edition = if let Some(isbn13) = &row.isbn13 {
        match crate::domain::catalog::create_edition(
            pool,
            &crate::models::CreateEditionInput {
                work_id: detail.work.id.clone(),
                isbn10: None,
                isbn13: Some(isbn13.clone()),
                publisher: None,
                published_at: None,
                language: None,
                page_count: None,
                binding: None,
            },
        )
        .await
        {
            Ok(e) => Some(e.id),
            Err(AppError::Conflict(_)) => None, // raced with another row
            Err(e) => return Err(e),
        }
    } else {
        let e = crate::domain::catalog::create_edition(
            pool,
            &crate::models::CreateEditionInput {
                work_id: detail.work.id.clone(),
                isbn10: None,
                isbn13: None,
                publisher: None,
                published_at: None,
                language: None,
                page_count: None,
                binding: None,
            },
        )
        .await?;
        Some(e.id)
    };
    Ok((detail.work.id, edition.unwrap_or_default()))
}

// ---------------------------------------------------------------------------
// Generic JSON import (docs/import-format.md): works -> editions -> copies.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonEdition {
    pub isbn13: Option<String>,
    pub isbn10: Option<String>,
    pub publisher: Option<String>,
    pub published_at: Option<String>,
    pub language: Option<String>,
    pub page_count: Option<i64>,
    pub binding: Option<String>,
    #[serde(default)]
    pub copies: Vec<JsonCopy>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonCopy {
    pub condition_grade: Option<String>,
    pub acquired_at: Option<String>,
    pub price_minor: Option<i64>,
    pub currency: Option<String>,
    pub store: Option<String>,
    pub acquisition_type: Option<String>,
    /// Location is matched against existing locations in later UI work; the
    /// demo dataset carries it for fidelity.
    #[allow(dead_code)]
    pub location_name: Option<String>,
    pub note: Option<String>,
    pub sentimental_score: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonWork {
    pub title: String,
    pub subtitle: Option<String>,
    #[serde(default)]
    pub author_names: Vec<String>,
    pub original_language: Option<String>,
    #[serde(default)]
    pub editions: Vec<JsonEdition>,
}

/// Import a JSON library snapshot. Rows are committed per-work; a failing work
/// is reported and skipped, never silently dropped.
pub async fn import_json(pool: &SqlitePool, json_text: &str) -> AppResult<ImportResult> {
    let works: Vec<JsonWork> = serde_json::from_str(json_text)
        .map_err(|e| AppError::Validation(format!("invalid import JSON: {e}")))?;

    let job_id = ids::new_id();
    sqlx::query("INSERT INTO import_jobs (id, source, started_at, status) VALUES (?1, 'json', ?2, 'running')")
        .bind(&job_id)
        .bind(ids::now_iso())
        .execute(pool)
        .await?;

    let mut result = ImportResult {
        imported_works: 0,
        imported_editions: 0,
        imported_copies: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    for (i, work) in works.into_iter().enumerate() {
        match import_json_work(pool, work).await {
            Ok((w, e, c)) => {
                result.imported_works += w as i64;
                result.imported_editions += e as i64;
                result.imported_copies += c as i64;
            }
            Err(err) => {
                result.errors.push(format!("work {}: {}", i + 1, err));
                result.skipped += 1;
            }
        }
    }

    sqlx::query("UPDATE import_jobs SET finished_at = ?2, status = 'completed', stats_json = ?3 WHERE id = ?1")
        .bind(&job_id)
        .bind(ids::now_iso())
        .bind(serde_json::to_string(&result).unwrap_or_default())
        .execute(pool)
        .await?;
    Ok(result)
}

async fn import_json_work(pool: &SqlitePool, work: JsonWork) -> AppResult<(bool, usize, usize)> {
    if work.title.trim().is_empty() {
        return Err(AppError::Validation("missing title".into()));
    }
    // Skip works that already exist (title+first author exact match).
    if !work.author_names.is_empty() {
        let dup = crate::domain::catalog::find_duplicates(
            pool,
            &crate::models::DuplicateQuery {
                isbn: None,
                title: Some(work.title.clone()),
                author_name: Some(work.author_names[0].clone()),
            },
        )
        .await?;
        if !dup.is_empty() {
            return Ok((false, 0, 0));
        }
    }
    let detail = crate::domain::catalog::create_work(
        pool,
        &crate::models::CreateWorkInput {
            title: work.title,
            subtitle: work.subtitle,
            original_language: work.original_language,
            description: None,
            author_names: work.author_names,
        },
    )
    .await?;

    let mut edition_count = 0usize;
    let mut copy_count = 0usize;
    for edition in work.editions {
        let edition_dto = crate::domain::catalog::create_edition(
            pool,
            &crate::models::CreateEditionInput {
                work_id: detail.work.id.clone(),
                isbn10: edition.isbn10,
                isbn13: edition.isbn13,
                publisher: edition.publisher,
                published_at: edition.published_at,
                language: edition.language,
                page_count: edition.page_count,
                binding: edition.binding,
            },
        )
        .await;
        let edition_dto = match edition_dto {
            Ok(e) => e,
            Err(AppError::Conflict(_)) => continue, // ISBN already present
            Err(e) => return Err(e),
        };
        edition_count += 1;
        for copy in edition.copies {
            if copy.condition_grade.is_some() || copy.price_minor.is_some() {
                // Clone up front: the acquisition reuses these fields.
                let acquired_at = copy.acquired_at.clone();
                let acquisition_type = copy.acquisition_type.clone();
                crate::domain::catalog::create_copy(
                    pool,
                    &crate::models::CreateCopyInput {
                        edition_id: edition_dto.id.clone(),
                        inventory_code: None,
                        location_id: None,
                        condition_grade: copy.condition_grade.unwrap_or_else(|| "good".into()),
                        acquisition_type: acquisition_type.clone(),
                        acquired_at: acquired_at.clone(),
                        note: copy.note,
                        sentimental_score: copy.sentimental_score,
                        acquisition: copy.price_minor.map(|price_minor| {
                            crate::models::CreateAcquisitionInput {
                                acquired_at,
                                price_minor: Some(price_minor),
                                currency: copy.currency.clone(),
                                shipping_minor: None,
                                tax_minor: None,
                                store: copy.store.clone(),
                                acquisition_type,
                                receipt_path: None,
                            }
                        }),
                    },
                )
                .await?;
                copy_count += 1;
            }
        }
    }
    Ok((true, edition_count, copy_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_memory;

    const CSV: &str = "Book Id,Title,Author,ISBN13,My Rating,Bookshelves,Date Read\n\
        1,The Hobbit,\"Tolkien, J.R.R.\",=\"9780547928227\",4,read,2024/03/05\n\
        2,Bad ISBN Book,Someone,=\"123\",0,to-read,\n\
        3,No ISBN Book,Other,,,read,2025/01/02\n";

    #[test]
    fn parses_goodreads_quoting_and_isbn_wrapper() {
        let preview = preview_goodreads_csv(CSV).unwrap();
        assert_eq!(preview.rows.len(), 3);
        let hobbit = &preview.rows[0];
        assert_eq!(hobbit.title, "The Hobbit");
        assert_eq!(hobbit.isbn13.as_deref(), Some("9780547928227"));
        assert_eq!(hobbit.rating, Some(4));
        assert_eq!(hobbit.shelf.as_deref(), Some("read"));
        assert!(hobbit.warnings.is_empty());

        assert!(preview.rows[1]
            .warnings
            .iter()
            .any(|w| w.contains("checksum")));
        assert!(preview.rows[2]
            .warnings
            .iter()
            .any(|w| w.contains("no ISBN")));
    }

    #[tokio::test]
    async fn commit_creates_works_and_reading_sessions() {
        let pool = connect_memory().await.unwrap();
        let file = std::env::temp_dir().join("shelfledger-test-goodreads.csv");
        std::fs::write(&file, CSV).unwrap();

        let preview = preview_goodreads_csv(CSV).unwrap();
        let preview = annotate_duplicates(&pool, preview).await.unwrap();
        assert!(preview.rows.iter().all(|r| r.duplicate_of.is_none()));

        let result = commit_goodreads_csv(
            &pool,
            file.to_str().unwrap(),
            &ImportCommitInput {
                row_numbers: vec![1, 2, 3],
                duplicate_policy: "skip".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(result.imported_works, 3);
        // Rows 1 and 3 create editions; row 2's failed-checksum ISBN imports
        // as a manual (no-ISBN) edition.
        assert_eq!(result.imported_editions, 3);
        assert_eq!(result.errors.len(), 0);

        // Reading sessions created for every shelf: rows 1+3 finished,
        // row 2 want_to_read.
        let sessions = crate::domain::reading::list_sessions(&pool, None)
            .await
            .unwrap();
        assert_eq!(sessions.len(), 3);
        let hobbit = sessions
            .iter()
            .find(|s| s.work_title == "The Hobbit")
            .unwrap();
        assert_eq!(hobbit.status, "finished");
        assert_eq!(hobbit.rating, Some(4));
        // Goodreads date read was preserved.
        assert_eq!(hobbit.finished_at.as_deref(), Some("2024-03-05"));

        // Re-import with duplicates resolved as "skip" imports nothing new.
        let again = commit_goodreads_csv(
            &pool,
            file.to_str().unwrap(),
            &ImportCommitInput {
                row_numbers: vec![1, 2, 3],
                duplicate_policy: "skip".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(again.imported_works, 0);
        assert_eq!(again.imported_editions, 0);
    }

    #[tokio::test]
    async fn json_import_roundtrip_with_copies() {
        let pool = connect_memory().await.unwrap();
        let json = serde_json::json!([
            {
                "title": "Imported Work",
                "authorNames": ["Author X"],
                "editions": [{
                    "isbn13": "9780306406157",
                    "publisher": "Pub",
                    "copies": [{
                        "conditionGrade": "like_new",
                        "priceMinor": 2500,
                        "currency": "RON",
                        "acquiredAt": "2026-01-15"
                    }]
                }]
            }
        ])
        .to_string();

        let result = import_json(&pool, &json).await.unwrap();
        assert_eq!(result.imported_works, 1);
        assert_eq!(result.imported_editions, 1);
        assert_eq!(result.imported_copies, 1);

        // Re-import skips duplicates.
        let again = import_json(&pool, &json).await.unwrap();
        assert_eq!(again.imported_works, 0);

        // The copy carries its acquisition.
        let copies = crate::repositories::copies::list_all_copies(&pool)
            .await
            .unwrap();
        assert_eq!(copies.len(), 1);
        assert_eq!(
            copies[0].acquisition.as_ref().unwrap().price_minor,
            Some(2500)
        );
    }

    #[tokio::test]
    async fn corrupt_json_is_a_validation_error() {
        let pool = connect_memory().await.unwrap();
        let err = import_json(&pool, "{ not json").await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn add_copy_policy_attaches_copies_to_existing_editions() {
        let pool = connect_memory().await.unwrap();
        let file = std::env::temp_dir().join("shelfledger-test-goodreads-addcopy.csv");
        std::fs::write(&file, CSV).unwrap();

        // First import creates the works.
        let first = commit_goodreads_csv(
            &pool,
            file.to_str().unwrap(),
            &ImportCommitInput {
                row_numbers: vec![1, 2, 3],
                duplicate_policy: "skip".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(first.imported_works, 3);

        // Second import with add_copy: every duplicate row attaches a copy to
        // the existing edition instead of being skipped or duplicated.
        let again = commit_goodreads_csv(
            &pool,
            file.to_str().unwrap(),
            &ImportCommitInput {
                row_numbers: vec![1, 2, 3],
                duplicate_policy: "add_copy".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(again.imported_works, 0);
        assert_eq!(again.imported_editions, 0);
        assert_eq!(again.imported_copies, 3);

        let copies = crate::repositories::copies::list_all_copies(&pool)
            .await
            .unwrap();
        assert_eq!(copies.len(), 3);
        // No new works appeared.
        assert_eq!(
            crate::repositories::works::list_work_summaries(&pool)
                .await
                .unwrap()
                .len(),
            3
        );
    }
}
