use super::works;
use crate::{
    error::AppResult,
    models::{CreateCopyInput, CreateEditionInput},
    sqlite_db as sqlx,
};
use sqlx::SqlitePool;
pub async fn update_edition(
    pool: &SqlitePool,
    id: &str,
    input: &CreateEditionInput,
    isbn10: Option<String>,
    isbn13: Option<String>,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE editions SET isbn10=?,isbn13=?,publisher=?,published_at=?,language=?,page_count=?,binding=? WHERE id=?")
        .bind(isbn10).bind(isbn13).bind(&input.publisher).bind(&input.published_at).bind(&input.language).bind(input.page_count).bind(&input.binding).bind(id).execute(&mut *tx).await?;
    works::sync_fts(&mut tx, &input.work_id).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn update_copy_details(
    pool: &SqlitePool,
    id: &str,
    input: &CreateCopyInput,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE copies SET inventory_code=?,location_id=?,condition_grade=?,acquisition_type=?,acquired_at=?,note=?,sentimental_score=?,updated_at=? WHERE id=?")
        .bind(&input.inventory_code).bind(&input.location_id).bind(&input.condition_grade).bind(&input.acquisition_type).bind(&input.acquired_at).bind(&input.note).bind(input.sentimental_score).bind(crate::ids::now_iso()).bind(id).execute(&mut *tx).await?;
    if let Some(a) = &input.acquisition {
        sqlx::query("INSERT INTO acquisitions (id,copy_id,acquired_at,price_minor,currency,shipping_minor,tax_minor,store,acquisition_type,receipt_path,created_at) VALUES (?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(copy_id) DO UPDATE SET acquired_at=excluded.acquired_at,price_minor=excluded.price_minor,currency=excluded.currency,shipping_minor=excluded.shipping_minor,tax_minor=excluded.tax_minor,store=excluded.store,acquisition_type=excluded.acquisition_type")
        .bind(crate::ids::new_id()).bind(id).bind(&a.acquired_at).bind(a.price_minor).bind(a.currency.as_deref().map(str::to_uppercase)).bind(a.shipping_minor).bind(a.tax_minor).bind(&a.store).bind(&a.acquisition_type).bind(&a.receipt_path).bind(crate::ids::now_iso()).execute(&mut *tx).await?;
    }
    let (work_id,): (String,) = sqlx::query_as("SELECT work_id FROM editions WHERE id=?")
        .bind(&input.edition_id)
        .fetch_one(&mut *tx)
        .await?;
    works::sync_fts(&mut tx, &work_id).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn set_work_tags(pool: &SqlitePool, id: &str, tags: &[String]) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM work_tags WHERE work_id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for tag in tags {
        sqlx::query("INSERT INTO tags (id,name) VALUES (?,?) ON CONFLICT(name) DO NOTHING")
            .bind(crate::ids::new_id())
            .bind(tag.trim())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT OR IGNORE INTO work_tags SELECT ?,id FROM tags WHERE name=?")
            .bind(id)
            .bind(tag.trim())
            .execute(&mut *tx)
            .await?;
    }
    works::sync_fts(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn list_work_tags(pool: &SqlitePool, id: &str) -> AppResult<Vec<String>> {
    let rows:Vec<(String,)>=sqlx::query_as("SELECT t.name FROM tags t JOIN work_tags wt ON wt.tag_id=t.id WHERE wt.work_id=? ORDER BY name").bind(id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(n,)| n).collect())
}
