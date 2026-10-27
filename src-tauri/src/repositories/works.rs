//! Work + author persistence and the FTS5 search index.
//! All SQL lives here; domain logic in `crate::domain`.

use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;

use crate::error::{AppError, AppResult};
use crate::ids;
use crate::models::{AuthorDto, WorkDto, WorkSummary};

pub async fn get_work<'a, E>(exec: E, id: &str) -> AppResult<Option<WorkDto>>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT id, title, subtitle, original_language, description, created_at, updated_at \
         FROM works WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(exec)
    .await?;
    Ok(row.map(
        |(id, title, subtitle, original_language, description, created_at, updated_at)| WorkDto {
            id,
            title,
            subtitle,
            original_language,
            description,
            created_at,
            updated_at,
        },
    ))
}

pub async fn list_author_names<'a, E>(exec: E, work_id: &str) -> AppResult<Vec<String>>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT a.name FROM work_authors wa JOIN authors a ON a.id = wa.author_id \
         WHERE wa.work_id = ?1 ORDER BY wa.sort_order",
    )
    .bind(work_id)
    .fetch_all(exec)
    .await?;
    Ok(rows.into_iter().map(|(n,)| n).collect())
}

/// Full author DTOs linked to a work, in catalog order.
pub async fn list_work_authors<'a, E>(exec: E, work_id: &str) -> AppResult<Vec<AuthorDto>>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.name, a.sort_name FROM work_authors wa \
         JOIN authors a ON a.id = wa.author_id WHERE wa.work_id = ?1 ORDER BY wa.sort_order",
    )
    .bind(work_id)
    .fetch_all(exec)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, sort_name)| AuthorDto {
            id,
            name,
            sort_name,
        })
        .collect())
}

const AUTHOR_FIND_SQL: &str = "SELECT id FROM authors WHERE lower(name) = lower(?) LIMIT 1";
const AUTHOR_INSERT_SQL: &str =
    "INSERT INTO authors (id, name, sort_name, created_at) VALUES (?, ?, NULL, ?)";

/// Boundary sanitizer for user-supplied author names (UI text, CSV imports):
/// trims, strips control characters (incl. NUL), enforces a length cap, and
/// rejects empty results. Every name is scrubbed here before it can reach a
/// query parameter — SQL uses bound parameters only, never interpolation.
pub fn sanitize_author_name(raw: &str) -> AppResult<String> {
    let cleaned: String = raw.trim().chars().filter(|c| !c.is_control()).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return Err(AppError::Validation("author name is required".into()));
    }
    if cleaned.chars().count() > 200 {
        return Err(AppError::Validation(
            "author name is too long (max 200 characters)".into(),
        ));
    }
    Ok(cleaned.to_string())
}

/// Find or create an author by case-insensitive name; returns the author id.
/// `name` is sanitized before any query; values are always bound (?), never
/// concatenated into SQL text.
pub async fn ensure_author(conn: &mut sqlx::SqliteConnection, name: &str) -> AppResult<String> {
    let name = sanitize_author_name(name)?;
    let existing: Option<(String,)> = sqlx::query_as(AUTHOR_FIND_SQL)
        .bind(&name)
        .fetch_optional(&mut *conn)
        .await?;
    if let Some((id,)) = existing {
        return Ok(id);
    }
    let id = ids::new_id();
    sqlx::query(AUTHOR_INSERT_SQL)
        .bind(&id)
        .bind(&name)
        .bind(ids::now_iso())
        .execute(&mut *conn)
        .await?;
    Ok(id)
}

pub async fn set_work_authors(
    tx: &mut Transaction<'_, Sqlite>,
    work_id: &str,
    author_names: &[String],
) -> AppResult<()> {
    sqlx::query("DELETE FROM work_authors WHERE work_id = ?1")
        .bind(work_id)
        .execute(&mut **tx)
        .await?;
    for (i, name) in author_names.iter().enumerate() {
        // Blank entries are dropped silently; malformed names are rejected
        // by the sanitizer inside ensure_author.
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        let author_id = ensure_author(&mut *tx, trimmed).await?;
        sqlx::query(
            "INSERT INTO work_authors (work_id, author_id, role, sort_order) \
             VALUES (?1, ?2, 'author', ?3) \
             ON CONFLICT (work_id, author_id) DO UPDATE SET sort_order = excluded.sort_order",
        )
        .bind(work_id)
        .bind(&author_id)
        .bind(i as i64)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn insert_work(
    tx: &mut Transaction<'_, Sqlite>,
    title: &str,
    subtitle: Option<&str>,
    original_language: Option<&str>,
    description: Option<&str>,
) -> AppResult<WorkDto> {
    let id = ids::new_id();
    let now = ids::now_iso();
    sqlx::query(
        "INSERT INTO works (id, title, subtitle, original_language, description, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
    )
    .bind(&id)
    .bind(title.trim())
    .bind(subtitle.map(str::trim).filter(|s| !s.is_empty()))
    .bind(original_language.map(str::trim).filter(|s| !s.is_empty()))
    .bind(description.map(str::trim).filter(|s| !s.is_empty()))
    .bind(&now)
    .execute(&mut **tx)
    .await?;
    get_work(&mut **tx, &id)
        .await?
        .ok_or_else(|| AppError::Other("work row vanished after insert".into()))
}

pub async fn update_work(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    title: &str,
    subtitle: Option<&str>,
    original_language: Option<&str>,
    description: Option<&str>,
) -> AppResult<bool> {
    let result = sqlx::query(
        "UPDATE works SET title = ?2, subtitle = ?3, original_language = ?4, description = ?5, \
         updated_at = ?6 WHERE id = ?1",
    )
    .bind(id)
    .bind(title.trim())
    .bind(subtitle.map(str::trim).filter(|s| !s.is_empty()))
    .bind(original_language.map(str::trim).filter(|s| !s.is_empty()))
    .bind(description.map(str::trim).filter(|s| !s.is_empty()))
    .bind(ids::now_iso())
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() > 0)
}

async fn work_authors_map<'a, E>(exec: E) -> AppResult<HashMap<String, Vec<String>>>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT wa.work_id, a.name FROM work_authors wa \
         JOIN authors a ON a.id = wa.author_id ORDER BY wa.sort_order",
    )
    .fetch_all(exec)
    .await?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for (work_id, name) in rows {
        map.entry(work_id).or_default().push(name);
    }
    Ok(map)
}

type SummaryRow = (String, String, Option<String>, Option<String>, i64, i64);

/// All works as compact summaries, newest first.
pub async fn list_work_summaries(pool: &SqlitePool) -> AppResult<Vec<WorkSummary>> {
    let rows: Vec<SummaryRow> = sqlx::query_as(
        "SELECT w.id, w.title, w.subtitle, \
           (SELECT e.cover_path FROM editions e WHERE e.work_id = w.id AND e.cover_path IS NOT NULL LIMIT 1), \
           (SELECT COUNT(*) FROM editions e WHERE e.work_id = w.id), \
           (SELECT COUNT(*) FROM copies c JOIN editions e2 ON c.edition_id = e2.id WHERE e2.work_id = w.id) \
         FROM works w ORDER BY w.created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    let authors = work_authors_map(pool).await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, title, subtitle, cover_path, edition_count, copy_count)| WorkSummary {
                author_names: authors.get(&id).cloned().unwrap_or_default(),
                id,
                title,
                subtitle,
                cover_path,
                edition_count,
                copy_count,
            },
        )
        .collect())
}

pub async fn get_work_summaries(pool: &SqlitePool, ids: &[String]) -> AppResult<Vec<WorkSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut summaries = list_work_summaries(pool).await?;
    summaries.retain(|s| ids.contains(&s.id));
    // Preserve the caller's (relevance/FTS rank) order.
    summaries.sort_by_key(|s| ids.iter().position(|i| i == &s.id).unwrap_or(usize::MAX));
    Ok(summaries)
}

// ---------------------------------------------------------------------------
// FTS5 index
// ---------------------------------------------------------------------------

/// Rebuild the FTS row for one work from current data. Covers title, authors,
/// ISBNs, copy notes, location names, and tag names (plan §8).
pub async fn sync_fts(conn: &mut sqlx::SqliteConnection, work_id: &str) -> AppResult<()> {
    let work = get_work(&mut *conn, work_id).await?;
    let (title, subtitle) = work
        .map(|w| (w.title, w.subtitle))
        .unwrap_or((String::new(), None));
    let authors = list_author_names(&mut *conn, work_id).await?;
    let isbns: Vec<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT isbn10, isbn13 FROM editions WHERE work_id = ?1")
            .bind(work_id)
            .fetch_all(&mut *conn)
            .await?;
    let isbns = isbns
        .into_iter()
        .flat_map(|(i10, i13)| [i10, i13].into_iter().flatten())
        .collect::<Vec<_>>()
        .join(" ");
    let notes: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT coalesce(c.note, '') FROM copies c \
         JOIN editions e ON c.edition_id = e.id WHERE e.work_id = ?1 AND c.note IS NOT NULL",
    )
    .bind(work_id)
    .fetch_all(&mut *conn)
    .await?;
    let location_names: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT coalesce(l.name, '') FROM copies c \
         JOIN editions e ON c.edition_id = e.id \
         LEFT JOIN locations l ON l.id = c.location_id \
         WHERE e.work_id = ?1 AND l.name IS NOT NULL",
    )
    .bind(work_id)
    .fetch_all(&mut *conn)
    .await?;
    let tag_names: Vec<(String,)> = sqlx::query_as(
        "SELECT t.name FROM work_tags wt JOIN tags t ON t.id = wt.tag_id \
         WHERE wt.work_id = ?1",
    )
    .bind(work_id)
    .fetch_all(&mut *conn)
    .await?;

    sqlx::query("DELETE FROM works_fts WHERE work_id = ?1")
        .bind(work_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "INSERT INTO works_fts (work_id, title, subtitle, author_names, isbns, notes, \
         location_names, tag_names) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(work_id)
    .bind(&title)
    .bind(subtitle.unwrap_or_default())
    .bind(authors.join(" "))
    .bind(&isbns)
    .bind(
        notes
            .into_iter()
            .map(|(n,)| n)
            .collect::<Vec<_>>()
            .join(" "),
    )
    .bind(
        location_names
            .into_iter()
            .map(|(n,)| n)
            .collect::<Vec<_>>()
            .join(" "),
    )
    .bind(
        tag_names
            .into_iter()
            .map(|(n,)| n)
            .collect::<Vec<_>>()
            .join(" "),
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Build a safe FTS5 MATCH expression from free text: each token becomes a
/// quoted prefix term, joined with AND. A single hyphenated token is fused
/// into one term only when it is ISBN-like ("978-0-306-40615-7" must match
/// the indexed "9780306406157"); hyphenated words ("error-correction") are
/// two terms, because that is how the tokenizer indexed them. Empty when no
/// usable tokens.
pub fn fts_match_query(query: &str) -> Option<String> {
    let trimmed = query.trim();
    // Hyphenated single token (no whitespace): ISBN-like values fuse into one
    // normalized term; anything else falls through to word splitting below.
    if !trimmed.contains(char::is_whitespace) && trimmed.contains('-') {
        let cleaned: String = trimmed
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        let digits = cleaned.chars().filter(|c| c.is_ascii_digit()).count();
        let isbn_like =
            (cleaned.len() == 13 && digits == 13) || (cleaned.len() == 10 && digits >= 9);
        if isbn_like {
            return (!cleaned.is_empty()).then(|| format!("\"{cleaned}\"*"));
        }
    }
    let terms: Vec<String> = trimmed
        .split_whitespace()
        .flat_map(|t| t.split('-'))
        .map(|t| {
            let cleaned: String = t
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase();
            format!("\"{cleaned}\"*")
        })
        .filter(|t| t != "\"\"*")
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

/// FTS search returning work ids in relevance order.
pub async fn search_fts_ids(pool: &SqlitePool, query: &str) -> AppResult<Vec<String>> {
    let Some(match_query) = fts_match_query(query) else {
        return Ok(Vec::new());
    };
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT work_id FROM works_fts WHERE works_fts MATCH ?1 ORDER BY rank LIMIT 200",
    )
    .bind(&match_query)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_memory;

    async fn seed() -> SqlitePool {
        let pool = connect_memory().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        insert_work(&mut tx, "American Gods", Some("A Novel"), None, None)
            .await
            .unwrap();
        let work_id: (String,) =
            sqlx::query_as("SELECT id FROM works WHERE title = 'American Gods'")
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        set_work_authors(&mut tx, &work_id.0, &["Neil Gaiman".into()])
            .await
            .unwrap();
        tx.commit().await.unwrap();
        pool
    }

    // Work ids are UUIDs, so we look them up by title in tests.
    async fn find_work_id(pool: &SqlitePool, title: &str) -> String {
        sqlx::query_as::<_, (String,)>("SELECT id FROM works WHERE title = ?1")
            .bind(title)
            .fetch_one(pool)
            .await
            .unwrap()
            .0
    }

    #[test]
    fn sanitize_author_name_scrubs_and_rejects() {
        // Control characters (incl. NUL) are stripped; surrounding whitespace
        // trimmed — what reaches bound query parameters is plain text.
        assert_eq!(
            sanitize_author_name("  Neil Gaiman \u{0000}").unwrap(),
            "Neil Gaiman"
        );
        assert_eq!(sanitize_author_name("Line\nBreak").unwrap(), "LineBreak");
        // Empty and over-long names are rejected outright.
        assert!(sanitize_author_name("   ").is_err());
        assert!(sanitize_author_name("\u{0000}\u{0001}").is_err());
        let long = "a".repeat(201);
        assert!(sanitize_author_name(&long).is_err());
    }

    #[tokio::test]
    async fn ensure_author_is_case_insensitive_and_idempotent() {
        let pool = seed().await;
        let mut tx = pool.begin().await.unwrap();
        let a = ensure_author(&mut tx, "Neil Gaiman").await.unwrap();
        let b = ensure_author(&mut tx, "neil gaiman").await.unwrap();
        assert_eq!(a, b);
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn fts_matches_author_prefix_terms() {
        let pool = seed().await;
        let work_id = find_work_id(&pool, "American Gods").await;
        let mut conn = pool.acquire().await.unwrap();
        sync_fts(&mut conn, &work_id).await.unwrap();

        assert_eq!(
            search_fts_ids(&pool, "gaiman").await.unwrap(),
            vec![work_id.clone()]
        );
        assert_eq!(
            search_fts_ids(&pool, "america").await.unwrap(),
            vec![work_id.clone()]
        );
        assert!(search_fts_ids(&pool, "tolkien").await.unwrap().is_empty());
        assert!(search_fts_ids(&pool, "\"'; DROP").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn summaries_include_author_names_and_counts() {
        let pool = seed().await;
        let work_id = find_work_id(&pool, "American Gods").await;
        let mut conn = pool.acquire().await.unwrap();
        sync_fts(&mut conn, &work_id).await.unwrap();
        let summaries = list_work_summaries(&pool).await.unwrap();
        assert_eq!(summaries.len(), 1);
        let s = &summaries[0];
        assert_eq!(s.title, "American Gods");
        assert_eq!(s.author_names, vec!["Neil Gaiman".to_string()]);
        assert_eq!(s.edition_count, 0);
        assert_eq!(s.copy_count, 0);
    }

    #[tokio::test]
    async fn fts_indexes_notes_locations_and_tags() {
        let pool = connect_memory().await.unwrap();
        use crate::domain::catalog;
        use crate::models::*;

        let work = catalog::create_work(
            &pool,
            &CreateWorkInput {
                title: "The Waves".into(),
                subtitle: None,
                original_language: None,
                description: None,
                author_names: vec!["Virginia Woolf".into()],
            },
        )
        .await
        .unwrap();
        let edition = catalog::create_edition(
            &pool,
            &CreateEditionInput {
                work_id: work.work.id.clone(),
                isbn10: None,
                isbn13: None,
                publisher: None,
                published_at: None,
                language: None,
                page_count: None,
                binding: None,
            },
        )
        .await
        .unwrap();
        let room = crate::repositories::locations::insert_location(
            &pool,
            &CreateLocationInput {
                name: "Wintergarten".into(),
                kind: "room".into(),
                parent_id: None,
                sort_order: None,
            },
        )
        .await
        .unwrap();
        catalog::create_copy(
            &pool,
            &CreateCopyInput {
                edition_id: edition.id.clone(),
                inventory_code: None,
                location_id: Some(room.id.clone()),
                condition_grade: "good".into(),
                acquisition_type: None,
                acquired_at: None,
                note: Some("signed at leipzig book fair".into()),
                sentimental_score: None,
                acquisition: None,
            },
        )
        .await
        .unwrap();

        // Index is maintained on writes; re-sync covers the copy changes.
        let mut conn = pool.acquire().await.unwrap();
        sync_fts(&mut conn, &work.work.id).await.unwrap();

        assert_eq!(
            search_fts_ids(&pool, "leipzig").await.unwrap(),
            vec![work.work.id.clone()],
            "copy notes must be searchable"
        );
        assert_eq!(
            search_fts_ids(&pool, "wintergarten").await.unwrap(),
            vec![work.work.id.clone()],
            "location names must be searchable"
        );

        // Tags join the index too.
        sqlx::query("INSERT INTO tags (id, name) VALUES ('t1', 'modernism')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO work_tags (work_id, tag_id) VALUES (?1, 't1')")
            .bind(&work.work.id)
            .execute(&pool)
            .await
            .unwrap();
        sync_fts(&mut conn, &work.work.id).await.unwrap();
        assert_eq!(
            search_fts_ids(&pool, "modernism").await.unwrap(),
            vec![work.work.id.clone()],
            "tag names must be searchable"
        );
    }

    #[tokio::test]
    async fn hyphenated_isbn_query_is_normalized() {
        // "978-0-306-40615-7" must become one prefix term matching the
        // indexed "9780306406157", not an AND of broken fragments.
        assert_eq!(
            fts_match_query("978-0-306-40615-7").as_deref(),
            Some("\"9780306406157\"*")
        );
        // Ordinary multi-word queries still AND their terms.
        assert_eq!(
            fts_match_query("gods gaiman").as_deref(),
            Some("\"gods\"* AND \"gaiman\"*")
        );
        // Hyphenated words are two terms, not one fused token.
        assert_eq!(
            fts_match_query("error-correction").as_deref(),
            Some("\"error\"* AND \"correction\"*")
        );
        assert_eq!(
            fts_match_query("sci-fi novel").as_deref(),
            Some("\"sci\"* AND \"fi\"* AND \"novel\"*")
        );
        // An ISBN-10 with an X check digit is still ISBN-like.
        assert_eq!(
            fts_match_query("0-8044-2957-X").as_deref(),
            Some("\"080442957x\"*")
        );
        assert_eq!(fts_match_query("   "), None);
    }

    #[tokio::test]
    async fn hyphenated_word_query_matches_indexed_tokens() {
        // Regression from the live drill: searching a hyphenated word in a
        // title ("error-correction") found nothing because the query was
        // fused into one token the index never contains.
        let pool = connect_memory().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        insert_work(&mut tx, "Error-correction coding", None, None, None)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let work_id = find_work_id(&pool, "Error-correction coding").await;
        let mut conn = pool.acquire().await.unwrap();
        sync_fts(&mut conn, &work_id).await.unwrap();

        assert_eq!(
            search_fts_ids(&pool, "error-correction").await.unwrap(),
            vec![work_id.clone()]
        );
        assert_eq!(
            search_fts_ids(&pool, "coding").await.unwrap(),
            vec![work_id]
        );
    }
}
