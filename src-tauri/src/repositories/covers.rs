use crate::{
    error::{AppError, AppResult},
    models::CoverImage,
    sqlite_db::{self as sqlx, SqlitePool},
};

pub async fn get(pool: &SqlitePool, path: &str) -> AppResult<Option<CoverImage>> {
    let Some(id) = path
        .strip_prefix("covers/")
        .and_then(|value| value.strip_suffix(".jpg"))
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
    else {
        return Ok(None);
    };
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT data_url, source, created_at FROM edition_covers WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(data_url, source, created_at)| CoverImage {
        data_url,
        source,
        created_at,
    }))
}

pub async fn edition_isbn(pool: &SqlitePool, edition_id: &str) -> AppResult<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT coalesce(isbn13, isbn10) FROM editions WHERE id = ?1")
            .bind(edition_id)
            .fetch_optional(pool)
            .await?;
    row.map(|(isbn,)| isbn)
        .ok_or_else(|| AppError::NotFound("edition".into()))
}

pub async fn save(
    pool: &SqlitePool,
    edition_id: &str,
    data_url: &str,
    source: &str,
    expected_isbn: Option<&str>,
) -> AppResult<String> {
    let mut tx = pool.begin().await?;
    if let Some(expected) = expected_isbn {
        let current: Option<(Option<String>,)> =
            sqlx::query_as("SELECT coalesce(isbn13,isbn10) FROM editions WHERE id=?1")
                .bind(edition_id)
                .fetch_optional(&mut *tx)
                .await?;
        if current.and_then(|(isbn,)| isbn).as_deref() != Some(expected) {
            return Err(AppError::Conflict(
                "The edition ISBN changed while fetching its cover. Try again.".into(),
            ));
        }
    }
    let id = crate::ids::new_id();
    let path = format!("covers/{id}.jpg");
    let updated = sqlx::query("UPDATE editions SET cover_path = ?2 WHERE id = ?1")
        .bind(edition_id)
        .bind(&path)
        .execute(&mut *tx)
        .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound("edition".into()));
    }
    sqlx::query("DELETE FROM edition_covers WHERE edition_id = ?1")
        .bind(edition_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO edition_covers(id, edition_id, data_url, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
        .bind(id).bind(edition_id).bind(data_url).bind(source).bind(crate::ids::now_iso())
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(path)
}

pub async fn remove(pool: &SqlitePool, edition_id: &str) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query("UPDATE editions SET cover_path = NULL WHERE id = ?1")
        .bind(edition_id)
        .execute(&mut *tx)
        .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound("edition".into()));
    }
    sqlx::query("DELETE FROM edition_covers WHERE edition_id = ?1")
        .bind(edition_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
