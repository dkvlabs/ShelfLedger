//! Edition persistence.

use crate::sqlite_db as sqlx;
use sqlx::{Sqlite, SqlitePool};

use crate::error::AppResult;
use crate::ids;
use crate::models::EditionDto;

type EditionRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

fn row_to_dto(
    (
        id,
        work_id,
        isbn10,
        isbn13,
        publisher,
        published_at,
        language,
        page_count,
        binding,
        cover_path,
        provider_ids_json,
        created_at,
    ): EditionRow,
) -> EditionDto {
    EditionDto {
        id,
        work_id,
        isbn10,
        isbn13,
        publisher,
        published_at,
        language,
        page_count,
        binding,
        cover_path,
        provider_ids_json,
        created_at,
        copies: Vec::new(),
    }
}

pub async fn get_edition<'a, E>(exec: E, id: &str) -> AppResult<Option<EditionDto>>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    let row = sqlx::query_as::<_, EditionRow>(
        "SELECT id, work_id, isbn10, isbn13, publisher, published_at, language, page_count, \
                binding, cover_path, provider_ids_json, created_at \
         FROM editions WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(exec)
    .await?;
    Ok(row.map(row_to_dto))
}

pub async fn list_editions_for_work(
    pool: &SqlitePool,
    work_id: &str,
) -> AppResult<Vec<EditionDto>> {
    let rows: Vec<EditionRow> = sqlx::query_as(
        "SELECT id, work_id, isbn10, isbn13, publisher, published_at, language, page_count, \
                binding, cover_path, provider_ids_json, created_at \
         FROM editions WHERE work_id = ?1 ORDER BY created_at",
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_dto).collect())
}

/// Find an existing edition by normalized ISBN (13 preferred, then 10).
pub async fn find_by_isbn<'a, E>(
    exec: E,
    isbn10: Option<&str>,
    isbn13: Option<&str>,
) -> AppResult<Option<EditionDto>>
where
    E: sqlx::Executor<'a, Database = Sqlite> + Copy,
{
    if let Some(i13) = isbn13 {
        let maybe_id: Option<(String,)> =
            sqlx::query_as("SELECT id FROM editions WHERE isbn13 = ?1 LIMIT 1")
                .bind(i13)
                .fetch_optional(exec)
                .await?;
        if let Some((id,)) = maybe_id {
            return get_edition(exec, &id).await;
        }
    }
    if let Some(i10) = isbn10 {
        let maybe_id: Option<(String,)> =
            sqlx::query_as("SELECT id FROM editions WHERE isbn10 = ?1 LIMIT 1")
                .bind(i10)
                .fetch_optional(exec)
                .await?;
        if let Some((id,)) = maybe_id {
            return get_edition(exec, &id).await;
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_edition(
    exec: impl sqlx::Executor<'_, Database = Sqlite>,
    work_id: &str,
    isbn10: Option<&str>,
    isbn13: Option<&str>,
    publisher: Option<&str>,
    published_at: Option<&str>,
    language: Option<&str>,
    page_count: Option<i64>,
    binding: Option<&str>,
    provider_ids_json: Option<&str>,
) -> AppResult<String> {
    let id = ids::new_id();
    sqlx::query(
        "INSERT INTO editions (id, work_id, isbn10, isbn13, publisher, published_at, language, \
         page_count, binding, cover_path, provider_ids_json, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11)",
    )
    .bind(&id)
    .bind(work_id)
    .bind(isbn10)
    .bind(isbn13)
    .bind(publisher.map(str::trim).filter(|s| !s.is_empty()))
    .bind(published_at)
    .bind(language.map(str::trim).filter(|s| !s.is_empty()))
    .bind(page_count)
    .bind(binding)
    .bind(provider_ids_json)
    .bind(ids::now_iso())
    .execute(exec)
    .await?;
    Ok(id)
}

#[allow(dead_code)] // cover caching is a Phase 1 backlog / P1 feature
pub async fn set_cover_path<'a, E>(exec: E, id: &str, cover_path: Option<&str>) -> AppResult<()>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    sqlx::query("UPDATE editions SET cover_path = ?2 WHERE id = ?1")
        .bind(id)
        .bind(cover_path)
        .execute(exec)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_memory;
    use crate::repositories::works;

    #[tokio::test]
    async fn insert_and_fetch_edition() {
        let pool = connect_memory().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let work = works::insert_work(&mut tx, "T", None, None, None)
            .await
            .unwrap();
        works::set_work_authors(&mut tx, &work.id, &[])
            .await
            .unwrap();
        let edition_id = insert_edition(
            &mut *tx,
            &work.id,
            Some("0306406152"),
            Some("9780306406157"),
            Some("Addison"),
            Some("1988"),
            Some("en"),
            Some(320),
            Some("hardcover"),
            None,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let edition = get_edition(&pool, &edition_id).await.unwrap().unwrap();
        assert_eq!(edition.isbn13.as_deref(), Some("9780306406157"));
        assert_eq!(edition.page_count, Some(320));
        assert!(edition.copies.is_empty());
        assert_eq!(
            list_editions_for_work(&pool, &work.id).await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn duplicate_isbn13_is_rejected_by_unique_index() {
        let pool = connect_memory().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let work = works::insert_work(&mut tx, "T", None, None, None)
            .await
            .unwrap();
        works::set_work_authors(&mut tx, &work.id, &[])
            .await
            .unwrap();
        insert_edition(
            &mut *tx,
            &work.id,
            None,
            Some("9780306406157"),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        let second = insert_edition(
            &mut *tx,
            &work.id,
            None,
            Some("9780306406157"),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await;
        let err = second.unwrap_err();
        assert!(
            crate::error::is_unique_violation(&err),
            "expected unique violation, got {err:?}"
        );
    }
}
