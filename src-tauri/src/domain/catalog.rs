//! Catalog domain services: enforce invariants (valid ISBNs, unique ISBNs,
//! valid references) before any write, and record activity events.

use crate::sqlite_db as sqlx;
use sqlx::SqlitePool;

use crate::error::{is_unique_violation, is_unique_violation_sqlx, AppError, AppResult};
use crate::isbn;
use crate::models::{
    CopyDto, CreateCopyInput, CreateEditionInput, CreateWorkInput, DuplicateQuery, EditionDto,
    UpdateCopyInput, UpdateWorkInput, WorkDetail, WorkSummary,
};
use crate::repositories::{copies, editions, works};

async fn record_event(
    pool: &SqlitePool,
    entity_type: &str,
    entity_id: &str,
    action: &str,
    payload: serde_json::Value,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO activity_events (id, entity_type, entity_id, action, occurred_at, payload_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(crate::ids::new_id())
    .bind(entity_type)
    .bind(entity_id)
    .bind(action)
    .bind(crate::ids::now_iso())
    .bind(payload.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_work_detail(pool: &SqlitePool, id: &str) -> AppResult<WorkDetail> {
    let work = works::get_work(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("work {id}")))?;
    let authors = works::list_work_authors(pool, id).await?;
    let edition_rows = editions::list_editions_for_work(pool, id).await?;
    let mut edition_dtos = Vec::with_capacity(edition_rows.len());
    for mut edition in edition_rows {
        edition.copies = copies::list_copies_for_edition(pool, &edition.id).await?;
        edition_dtos.push(edition);
    }
    Ok(WorkDetail {
        work,
        authors,
        editions: edition_dtos,
    })
}

pub async fn create_work(pool: &SqlitePool, input: &CreateWorkInput) -> AppResult<WorkDetail> {
    input.validate().map_err(AppError::Validation)?;
    let mut tx = pool.begin().await?;
    let work = works::insert_work(
        &mut tx,
        &input.title,
        input.subtitle.as_deref(),
        input.original_language.as_deref(),
        input.description.as_deref(),
    )
    .await?;
    works::set_work_authors(&mut tx, &work.id, &input.author_names).await?;
    works::sync_fts(&mut tx, &work.id).await?;
    tx.commit().await?;
    record_event(
        pool,
        "work",
        &work.id,
        "created",
        serde_json::json!({ "title": work.title }),
    )
    .await?;
    get_work_detail(pool, &work.id).await
}

pub async fn update_work(
    pool: &SqlitePool,
    id: &str,
    input: &UpdateWorkInput,
) -> AppResult<WorkDetail> {
    input.validate().map_err(AppError::Validation)?;
    let mut tx = pool.begin().await?;
    if !works::update_work(
        &mut tx,
        id,
        &input.title,
        input.subtitle.as_deref(),
        input.original_language.as_deref(),
        input.description.as_deref(),
    )
    .await?
    {
        return Err(AppError::NotFound(format!("work {id}")));
    }
    works::set_work_authors(&mut tx, id, &input.author_names).await?;
    works::sync_fts(&mut tx, id).await?;
    tx.commit().await?;
    record_event(pool, "work", id, "updated", serde_json::json!({})).await?;
    get_work_detail(pool, id).await
}

/// Validate ISBNs (checksum required when present), fill in the missing form
/// via conversion, and reject a duplicate ISBN with a user-facing conflict.
fn normalize_edition_isbns(
    isbn10_in: Option<&String>,
    isbn13_in: Option<&String>,
) -> AppResult<(Option<String>, Option<String>)> {
    let n13 = isbn13_in
        .map(|s| isbn::normalize(s))
        .filter(|s| !s.is_empty());
    if let Some(s) = &n13 {
        if !isbn::is_valid_isbn13(s) {
            return Err(AppError::Validation(format!(
                "invalid ISBN-13 (checksum failed): {s}"
            )));
        }
    }
    let n10 = isbn10_in
        .map(|s| isbn::normalize(s))
        .filter(|s| !s.is_empty());
    if let Some(s) = &n10 {
        if !isbn::is_valid_isbn10(s) {
            return Err(AppError::Validation(format!(
                "invalid ISBN-10 (checksum failed): {s}"
            )));
        }
    }
    if let (Some(a), Some(b)) = (&n10, &n13) {
        if isbn::isbn10_to_isbn13(a).as_ref() != Some(b) {
            return Err(AppError::Validation(
                "ISBN-10 and ISBN-13 identify different editions".into(),
            ));
        }
    }
    let isbn13 = n13.or_else(|| n10.as_deref().and_then(isbn::isbn10_to_isbn13));
    let isbn10 = n10.or_else(|| isbn13.as_deref().and_then(isbn::isbn13_to_isbn10));
    Ok((isbn10, isbn13))
}

/// Workflow A ("Add book"): create work + edition + optional owned copy in ONE
/// transaction — a failure at any step rolls back everything, so no orphan
/// work is ever left behind. Duplicate ISBNs either surface as a `Conflict`
/// or (with `drop_conflicting_isbn`, the user's "add anyway" choice) are
/// dropped from the new edition and reported as a warning — ISBNs are never
/// cloned across editions.
pub async fn create_book(
    pool: &SqlitePool,
    input: &crate::models::AddBookInput,
) -> AppResult<crate::models::AddBookResult> {
    input.validate().map_err(AppError::Validation)?;
    let (mut isbn10, mut isbn13) =
        normalize_edition_isbns(input.isbn10.as_ref(), input.isbn13.as_ref())?;

    let mut warning = None;
    if editions::find_by_isbn(pool, isbn10.as_deref(), isbn13.as_deref())
        .await?
        .is_some()
    {
        if input.drop_conflicting_isbn {
            warning = Some(
                "This ISBN already belongs to another edition in your library; the new \
                 edition was created without it."
                    .to_string(),
            );
            isbn10 = None;
            isbn13 = None;
        } else {
            return Err(AppError::Conflict(
                "an edition with this ISBN already exists in your library".into(),
            ));
        }
    }

    let mut tx = pool.begin().await?;
    let work = works::insert_work(
        &mut tx,
        &input.title,
        input.subtitle.as_deref(),
        input.original_language.as_deref(),
        input.description.as_deref(),
    )
    .await?;
    works::set_work_authors(&mut tx, &work.id, &input.author_names).await?;
    let edition_id = editions::insert_edition(
        &mut *tx,
        &work.id,
        isbn10.as_deref(),
        isbn13.as_deref(),
        input.publisher.as_deref(),
        input.published_at.as_deref(),
        input.language.as_deref(),
        input.page_count,
        input.binding.as_deref(),
        None,
    )
    .await?;

    let mut copy_id = None;
    if let Some(copy) = &input.copy {
        let acquired_at = copy.acquired_at.clone().or_else(|| {
            copy.acquisition
                .as_ref()
                .and_then(|a| a.acquired_at.clone())
        });
        copy_id = Some(
            copies::insert_copy(
                &mut tx,
                &edition_id,
                copy.inventory_code.as_deref(),
                copy.location_id.as_deref(),
                &copy.condition_grade,
                copy.acquisition_type.as_deref(),
                acquired_at.as_deref(),
                copy.note.as_deref(),
                copy.sentimental_score,
                copy.acquisition.as_ref(),
            )
            .await?,
        );
    }

    works::sync_fts(&mut tx, &work.id).await?;
    tx.commit().await.map_err(|e| {
        // A concurrent ISBN insert can still trip the unique index; the
        // transaction has rolled back, so nothing orphaned.
        if is_unique_violation_sqlx(&e) {
            AppError::Conflict("an edition with this ISBN already exists in your library".into())
        } else {
            AppError::from(e)
        }
    })?;

    record_event(
        pool,
        "work",
        &work.id,
        "created",
        serde_json::json!({ "title": work.title }),
    )
    .await?;
    record_event(
        pool,
        "edition",
        &edition_id,
        "created",
        serde_json::json!({ "workId": work.id, "isbn13": isbn13 }),
    )
    .await?;
    if let Some(id) = &copy_id {
        record_event(
            pool,
            "copy",
            id,
            "acquired",
            serde_json::json!({ "editionId": edition_id }),
        )
        .await?;
    }

    Ok(crate::models::AddBookResult {
        work_id: work.id,
        edition_id,
        copy_id,
        warning,
    })
}

pub async fn create_edition(
    pool: &SqlitePool,
    input: &CreateEditionInput,
) -> AppResult<EditionDto> {
    input.validate().map_err(AppError::Validation)?;
    let (isbn10, isbn13) = normalize_edition_isbns(input.isbn10.as_ref(), input.isbn13.as_ref())?;
    if works::get_work(pool, &input.work_id).await?.is_none() {
        return Err(AppError::NotFound(format!("work {}", input.work_id)));
    }
    if editions::find_by_isbn(pool, isbn10.as_deref(), isbn13.as_deref())
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(
            "an edition with this ISBN already exists in your library".into(),
        ));
    }
    let id = editions::insert_edition(
        pool,
        &input.work_id,
        isbn10.as_deref(),
        isbn13.as_deref(),
        input.publisher.as_deref(),
        input.published_at.as_deref(),
        input.language.as_deref(),
        input.page_count,
        input.binding.as_deref(),
        None,
    )
    .await
    .map_err(|e| {
        if is_unique_violation(&e) {
            AppError::Conflict("an edition with this ISBN already exists in your library".into())
        } else {
            e
        }
    })?;
    let mut conn = pool.acquire().await?;
    works::sync_fts(&mut conn, &input.work_id).await?;
    record_event(
        pool,
        "edition",
        &id,
        "created",
        serde_json::json!({ "workId": input.work_id, "isbn13": isbn13 }),
    )
    .await?;
    editions::get_edition(pool, &id)
        .await?
        .ok_or_else(|| AppError::Other("edition row vanished after insert".into()))
}

pub async fn create_copy(pool: &SqlitePool, input: &CreateCopyInput) -> AppResult<CopyDto> {
    input.validate().map_err(AppError::Validation)?;
    if editions::get_edition(pool, &input.edition_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!("edition {}", input.edition_id)));
    }
    let acquisition = input.acquisition.clone();
    let acquired_at = input
        .acquired_at
        .clone()
        .or_else(|| acquisition.as_ref().and_then(|a| a.acquired_at.clone()));

    let mut tx = pool.begin().await?;
    let copy_id = copies::insert_copy(
        &mut tx,
        &input.edition_id,
        input.inventory_code.as_deref(),
        input.location_id.as_deref(),
        &input.condition_grade,
        input.acquisition_type.as_deref(),
        acquired_at.as_deref(),
        input.note.as_deref(),
        input.sentimental_score,
        acquisition.as_ref(),
    )
    .await
    .map_err(|e| {
        if crate::error::is_foreign_key_violation(&e) {
            AppError::Validation("referenced edition or location does not exist".into())
        } else {
            e
        }
    })?;
    let work_id: (String,) = sqlx::query_as("SELECT work_id FROM editions WHERE id=?")
        .bind(&input.edition_id)
        .fetch_one(&mut *tx)
        .await?;
    works::sync_fts(&mut tx, &work_id.0).await?;
    tx.commit().await?;
    record_event(
        pool,
        "copy",
        &copy_id,
        "acquired",
        serde_json::json!({ "editionId": input.edition_id }),
    )
    .await?;
    copies::get_copy(pool, &copy_id)
        .await?
        .ok_or_else(|| AppError::Other("copy row vanished after insert".into()))
}

pub async fn update_copy(
    pool: &SqlitePool,
    id: &str,
    input: &UpdateCopyInput,
) -> AppResult<CopyDto> {
    input.validate().map_err(AppError::Validation)?;
    if !copies::update_copy(pool, id, input).await? {
        return Err(AppError::NotFound(format!("active copy {id}")));
    }
    record_event(pool, "copy", id, "updated", serde_json::json!({})).await?;
    copies::get_copy(pool, id)
        .await?
        .ok_or_else(|| AppError::Other("copy row vanished after update".into()))
}

pub async fn archive_copy(pool: &SqlitePool, id: &str) -> AppResult<()> {
    if !copies::archive_copy(pool, id).await? {
        return Err(AppError::NotFound(format!("active copy {id}")));
    }
    record_event(pool, "copy", id, "archived", serde_json::json!({})).await?;
    Ok(())
}

/// Duplicate detection by ISBN (normalized + checksum-valid) or by
/// case-insensitive title with optional author narrowing.
pub async fn find_duplicates(
    pool: &SqlitePool,
    query: &DuplicateQuery,
) -> AppResult<Vec<WorkSummary>> {
    if let Some(raw_isbn) = query.isbn.as_deref().filter(|s| !s.trim().is_empty()) {
        // An unparseable ISBN simply has no known duplicates.
        let Some(n) = isbn::parse(raw_isbn) else {
            return Ok(Vec::new());
        };
        if let Some(edition) =
            editions::find_by_isbn(pool, n.isbn10.as_deref(), n.isbn13.as_deref()).await?
        {
            return works::get_work_summaries(pool, &[edition.work_id]).await;
        }
        return Ok(Vec::new());
    }

    let Some(title) = query
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(Vec::new());
    };
    let ids: Vec<(String,)> =
        sqlx::query_as("SELECT id FROM works WHERE lower(trim(title)) = lower(?1)")
            .bind(title)
            .fetch_all(pool)
            .await?;
    let mut matched = Vec::new();
    for (id,) in ids {
        if let Some(author) = query
            .author_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let names = works::list_author_names(pool, &id).await?;
            let author_lower = author.to_lowercase();
            if !names
                .iter()
                .any(|n| n.to_lowercase().contains(&author_lower))
            {
                continue;
            }
        }
        matched.push(id);
    }
    works::get_work_summaries(pool, &matched).await
}

pub async fn unarchive_copy(pool: &SqlitePool, id: &str) -> AppResult<()> {
    if !copies::unarchive_copy(pool, id).await? {
        return Err(AppError::NotFound("archived copy".into()));
    }
    record_event(pool, "copy", id, "unarchived", serde_json::json!({})).await
}

pub async fn update_edition(
    pool: &SqlitePool,
    id: &str,
    input: &CreateEditionInput,
) -> AppResult<EditionDto> {
    input.validate().map_err(AppError::Validation)?;
    let existing = editions::get_edition(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("edition".into()))?;
    if existing.work_id != input.work_id {
        return Err(AppError::Validation(
            "an edition cannot be moved to another work".into(),
        ));
    }
    let (isbn10, isbn13) = normalize_edition_isbns(input.isbn10.as_ref(), input.isbn13.as_ref())?;
    crate::repositories::catalog_edits::update_edition(pool, id, input, isbn10, isbn13)
        .await
        .map_err(|e| {
            if let AppError::Database(ref db) = e {
                if is_unique_violation_sqlx(db) {
                    return AppError::Conflict("ISBN belongs to another edition".into());
                }
            }
            e
        })?;
    record_event(pool, "edition", id, "updated", serde_json::json!({})).await?;
    editions::get_edition(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("edition".into()))
}

pub async fn update_copy_details(
    pool: &SqlitePool,
    id: &str,
    input: &CreateCopyInput,
) -> AppResult<CopyDto> {
    input.validate().map_err(AppError::Validation)?;
    let existing = copies::get_copy(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("copy".into()))?;
    if existing.archived_at.is_some() || existing.edition_id != input.edition_id {
        return Err(AppError::Validation(
            "cannot move or edit an archived copy".into(),
        ));
    }
    crate::repositories::catalog_edits::update_copy_details(pool, id, input).await?;
    record_event(pool, "copy", id, "updated", serde_json::json!({})).await?;
    copies::get_copy(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("copy".into()))
}

pub async fn set_work_tags(pool: &SqlitePool, id: &str, tags: &[String]) -> AppResult<()> {
    if tags.len() > 100 || tags.iter().any(|v| v.trim().is_empty() || v.len() > 100) {
        return Err(AppError::Validation("invalid tags".into()));
    }
    if works::get_work(pool, id).await?.is_none() {
        return Err(AppError::NotFound("work".into()));
    }
    crate::repositories::catalog_edits::set_work_tags(pool, id, tags).await
}
pub async fn list_work_tags(pool: &SqlitePool, id: &str) -> AppResult<Vec<String>> {
    crate::repositories::catalog_edits::list_work_tags(pool, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_memory;
    use crate::models::{CreateAcquisitionInput, CreateLocationInput};
    use crate::repositories::locations;

    async fn pool() -> SqlitePool {
        connect_memory().await.unwrap()
    }

    fn work_input(title: &str, authors: &[&str]) -> CreateWorkInput {
        CreateWorkInput {
            title: title.into(),
            subtitle: None,
            original_language: None,
            description: None,
            author_names: authors.iter().map(|s| s.to_string()).collect(),
        }
    }

    async fn seed_work(pool: &SqlitePool) -> WorkDetail {
        create_work(pool, &work_input("American Gods", &["Neil Gaiman"]))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn create_work_persists_authors_and_searches() {
        let pool = pool().await;
        let detail = seed_work(&pool).await;
        assert_eq!(detail.authors.len(), 1);
        assert_eq!(detail.authors[0].name, "Neil Gaiman");
        // FTS index was synced on create.
        let ids = works::search_fts_ids(&pool, "gaiman").await.unwrap();
        assert_eq!(ids, vec![detail.work.id]);
    }

    #[tokio::test]
    async fn empty_title_is_rejected() {
        let pool = pool().await;
        let err = create_work(&pool, &work_input("  ", &[]))
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn edition_isbn_validation_and_duplicates() {
        let pool = pool().await;
        let detail = seed_work(&pool).await;

        let bad = CreateEditionInput {
            work_id: detail.work.id.clone(),
            isbn10: None,
            isbn13: Some("9780306406158".into()), // bad checksum
            publisher: None,
            published_at: None,
            language: None,
            page_count: None,
            binding: None,
        };
        let err = create_edition(&pool, &bad).await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");

        let good = CreateEditionInput {
            work_id: detail.work.id.clone(),
            isbn10: Some("0-19-852663-6".into()),
            isbn13: None, // filled by conversion
            publisher: Some("OUP".into()),
            published_at: None,
            language: None,
            page_count: None,
            binding: None,
        };
        let edition = create_edition(&pool, &good).await.unwrap();
        assert_eq!(edition.isbn13.as_deref(), Some("9780198526636"));

        // Same ISBN again -> Conflict.
        let dup = CreateEditionInput {
            work_id: detail.work.id.clone(),
            isbn10: None,
            isbn13: Some("9780198526636".into()),
            publisher: None,
            published_at: None,
            language: None,
            page_count: None,
            binding: None,
        };
        let err = create_edition(&pool, &dup).await.unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)), "got {err:?}");

        // Manual edition without ISBN is allowed.
        let manual = CreateEditionInput {
            work_id: detail.work.id.clone(),
            isbn10: None,
            isbn13: None,
            publisher: None,
            published_at: None,
            language: None,
            page_count: None,
            binding: None,
        };
        let manual = create_edition(&pool, &manual).await.unwrap();
        assert!(manual.isbn13.is_none());
    }

    #[tokio::test]
    async fn duplicate_detection_by_isbn_and_title() {
        let pool = pool().await;
        let detail = seed_work(&pool).await;
        let edition = create_edition(
            &pool,
            &CreateEditionInput {
                work_id: detail.work.id.clone(),
                isbn10: None,
                isbn13: Some("9780306406157".into()),
                publisher: None,
                published_at: None,
                language: None,
                page_count: None,
                binding: None,
            },
        )
        .await
        .unwrap();

        let by_isbn = find_duplicates(
            &pool,
            &DuplicateQuery {
                isbn: Some("978-0-306-40615-7".into()),
                title: None,
                author_name: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(by_isbn.len(), 1);
        assert_eq!(by_isbn[0].id, detail.work.id);
        assert_eq!(
            editions::get_edition(&pool, &edition.id)
                .await
                .unwrap()
                .unwrap()
                .work_id,
            detail.work.id
        );

        let by_title = find_duplicates(
            &pool,
            &DuplicateQuery {
                isbn: None,
                title: Some("american gods".into()),
                author_name: Some("gaiman".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(by_title.len(), 1);

        let author_mismatch = find_duplicates(
            &pool,
            &DuplicateQuery {
                isbn: None,
                title: Some("american gods".into()),
                author_name: Some("tolkien".into()),
            },
        )
        .await
        .unwrap();
        assert!(author_mismatch.is_empty());

        let invalid_isbn = find_duplicates(
            &pool,
            &DuplicateQuery {
                isbn: Some("garbage".into()),
                title: None,
                author_name: None,
            },
        )
        .await
        .unwrap();
        assert!(invalid_isbn.is_empty());
    }

    #[tokio::test]
    async fn copies_carry_acquisition_and_default_status() {
        let pool = pool().await;
        let detail = seed_work(&pool).await;
        let edition = create_edition(
            &pool,
            &CreateEditionInput {
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
        .await
        .unwrap();

        let room = locations::insert_location(
            &pool,
            &CreateLocationInput {
                name: "Office".into(),
                kind: "room".into(),
                parent_id: None,
                sort_order: None,
            },
        )
        .await
        .unwrap();

        let copy = create_copy(
            &pool,
            &CreateCopyInput {
                edition_id: edition.id.clone(),
                inventory_code: Some("A-001".into()),
                location_id: Some(room.id.clone()),
                condition_grade: "like_new".into(),
                acquisition_type: Some("bought".into()),
                acquired_at: Some("2026-10-23".into()),
                note: None,
                sentimental_score: Some(4),
                acquisition: Some(CreateAcquisitionInput {
                    acquired_at: Some("2026-10-23".into()),
                    price_minor: Some(8999),
                    currency: Some("RON".into()),
                    shipping_minor: Some(1500),
                    tax_minor: None,
                    store: Some("Carturesti".into()),
                    acquisition_type: Some("bought".into()),
                    receipt_path: None,
                }),
            },
        )
        .await
        .unwrap();

        assert_eq!(copy.ownership_status, "owned");
        assert_eq!(copy.location_name.as_deref(), Some("Office"));
        let acq = copy.acquisition.expect("acquisition stored");
        assert_eq!(acq.price_minor, Some(8999));
        assert_eq!(acq.currency.as_deref(), Some("RON"));
        assert_eq!(acq.shipping_minor, Some(1500));
        assert_eq!(copy.created_at, copy.updated_at);

        // Price without currency is rejected.
        let err = create_copy(
            &pool,
            &CreateCopyInput {
                edition_id: edition.id.clone(),
                inventory_code: None,
                location_id: None,
                condition_grade: "good".into(),
                acquisition_type: None,
                acquired_at: None,
                note: None,
                sentimental_score: None,
                acquisition: Some(CreateAcquisitionInput {
                    acquired_at: None,
                    price_minor: Some(100),
                    currency: None,
                    shipping_minor: None,
                    tax_minor: None,
                    store: None,
                    acquisition_type: None,
                    receipt_path: None,
                }),
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");

        // Unknown location rejected via FK mapping.
        let err = create_copy(
            &pool,
            &CreateCopyInput {
                edition_id: edition.id.clone(),
                inventory_code: None,
                location_id: Some("missing".into()),
                condition_grade: "good".into(),
                acquisition_type: None,
                acquired_at: None,
                note: None,
                sentimental_score: None,
                acquisition: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn archive_is_soft_delete() {
        let pool = pool().await;
        let detail = seed_work(&pool).await;
        let edition = create_edition(
            &pool,
            &CreateEditionInput {
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
        .await
        .unwrap();
        let copy = create_copy(
            &pool,
            &CreateCopyInput {
                edition_id: edition.id,
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
        .await
        .unwrap();

        archive_copy(&pool, &copy.id).await.unwrap();
        let archived = copies::get_copy(&pool, &copy.id).await.unwrap().unwrap();
        assert!(archived.archived_at.is_some());
        // History is retained; row still fetchable.
        assert_eq!(archived.id, copy.id);
        // Archiving twice -> NotFound.
        let err = archive_copy(&pool, &copy.id).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn update_copy_changes_location_and_condition() {
        let pool = pool().await;
        let detail = seed_work(&pool).await;
        let edition = create_edition(
            &pool,
            &CreateEditionInput {
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
        .await
        .unwrap();
        let copy = create_copy(
            &pool,
            &CreateCopyInput {
                edition_id: edition.id,
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
        .await
        .unwrap();

        let updated = update_copy(
            &pool,
            &copy.id,
            &UpdateCopyInput {
                location_id: None,
                condition_grade: Some("poor".into()),
                note: Some(Some("water damage".into())),
                sentimental_score: Some(Some(1)),
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.condition_grade, "poor");
        assert_eq!(updated.note.as_deref(), Some("water damage"));
    }

    #[tokio::test]
    async fn update_work_edits_fields_and_authors() {
        let pool = pool().await;
        let detail = seed_work(&pool).await;

        let updated = update_work(
            &pool,
            &detail.work.id,
            &UpdateWorkInput {
                title: "American Gods: Revised".into(),
                subtitle: Some("A Novel".into()),
                original_language: Some("en".into()),
                description: Some("desc".into()),
                author_names: vec!["Neil Gaiman".into(), "M. Somebody".into()],
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.work.title, "American Gods: Revised");
        assert_eq!(updated.work.description.as_deref(), Some("desc"));
        assert_eq!(updated.authors.len(), 2);
        // FTS reflects the rename.
        let hits = crate::repositories::works::search_fts_ids(&pool, "revised")
            .await
            .unwrap();
        assert_eq!(hits, vec![detail.work.id]);

        // Unknown work -> NotFound.
        let err = update_work(
            &pool,
            "missing",
            &UpdateWorkInput {
                title: "x".into(),
                subtitle: None,
                original_language: None,
                description: None,
                author_names: vec![],
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn add_book_is_atomic_and_handles_isbn_conflicts() {
        let pool = pool().await;
        use crate::models::{AddBookCopyInput, AddBookInput};

        let input = AddBookInput {
            title: "Kindred".into(),
            subtitle: None,
            original_language: None,
            description: None,
            author_names: vec!["Octavia E. Butler".into()],
            isbn10: None,
            isbn13: Some("978-0-8070-8369-7".into()),
            publisher: None,
            published_at: None,
            language: None,
            page_count: None,
            binding: None,
            copy: Some(AddBookCopyInput {
                inventory_code: None,
                location_id: None,
                condition_grade: "good".into(),
                acquisition_type: Some("bought".into()),
                acquired_at: Some("2026-01-01".into()),
                note: None,
                sentimental_score: None,
                acquisition: Some(CreateAcquisitionInput {
                    acquired_at: Some("2026-01-01".into()),
                    price_minor: Some(4200),
                    currency: Some("RON".into()),
                    shipping_minor: None,
                    tax_minor: None,
                    store: None,
                    acquisition_type: Some("bought".into()),
                    receipt_path: None,
                }),
            }),
            drop_conflicting_isbn: false,
        };
        let result = create_book(&pool, &input).await.unwrap();
        assert!(result.copy_id.is_some());
        assert!(result.warning.is_none());
        let copy = crate::repositories::copies::get_copy(&pool, result.copy_id.as_deref().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(copy.acquisition.as_ref().unwrap().price_minor, Some(4200));

        // Same ISBN again -> Conflict, and NO orphan work is created
        // (transactional semantics).
        let mut dup = input.clone();
        dup.title = "Kindred (duplicate attempt)".into();
        let err = create_book(&pool, &dup).await.unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)), "got {err:?}");
        let summaries = crate::repositories::works::list_work_summaries(&pool)
            .await
            .unwrap();
        assert_eq!(
            summaries.len(),
            1,
            "failed add_book must not leave an orphan work"
        );

        // "Add anyway": the work is created, but the conflicting ISBN is
        // dropped from the new edition (never cloned) and reported.
        let mut anyway = dup;
        anyway.drop_conflicting_isbn = true;
        let result2 = create_book(&pool, &anyway).await.unwrap();
        assert!(result2.warning.is_some());
        let detail = get_work_detail(&pool, &result2.work_id).await.unwrap();
        assert!(
            detail.editions[0].isbn13.is_none(),
            "conflicting ISBN must be dropped"
        );
        assert_eq!(
            crate::repositories::works::list_work_summaries(&pool)
                .await
                .unwrap()
                .len(),
            2
        );

        // Validation failure before the transaction leaves no rows either.
        let mut bad = input;
        bad.title = "  ".into();
        assert!(create_book(&pool, &bad).await.is_err());
        assert_eq!(
            crate::repositories::works::list_work_summaries(&pool)
                .await
                .unwrap()
                .len(),
            2
        );
    }
}
