//! Copy persistence: the owned physical item plus its acquisition record.

use sqlx::{Sqlite, SqlitePool};

use crate::error::AppResult;
use crate::ids;
use crate::models::{AcquisitionDto, CopyDto};

const COPY_COLUMNS: &str =
    "c.id, c.edition_id, c.inventory_code, c.location_id, l.name, c.condition_grade, \
     c.ownership_status, c.acquisition_type, c.acquired_at, c.note, c.sentimental_score, \
     c.archived_at, c.created_at, c.updated_at";

type CopyRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    String,
    String,
);

fn row_to_dto(
    (
        id,
        edition_id,
        inventory_code,
        location_id,
        location_name,
        condition_grade,
        ownership_status,
        acquisition_type,
        acquired_at,
        note,
        sentimental_score,
        archived_at,
        created_at,
        updated_at,
    ): CopyRow,
) -> CopyDto {
    CopyDto {
        id,
        edition_id,
        inventory_code,
        location_id,
        location_name,
        condition_grade,
        ownership_status,
        acquisition_type,
        acquired_at,
        note,
        sentimental_score,
        archived_at,
        created_at,
        updated_at,
        acquisition: None,
    }
}

type AcquisitionRow = (
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
);

async fn attach_acquisitions<'a, E>(exec: E, copies: &mut [CopyDto]) -> AppResult<()>
where
    E: sqlx::Executor<'a, Database = Sqlite> + Copy,
{
    if copies.is_empty() {
        return Ok(());
    }
    for copy in copies.iter_mut() {
        let row: Option<AcquisitionRow> = sqlx::query_as(
            "SELECT id, copy_id, acquired_at, price_minor, currency, shipping_minor, tax_minor, \
                    store, acquisition_type, receipt_path \
             FROM acquisitions WHERE copy_id = ?1",
        )
        .bind(&copy.id)
        .fetch_optional(exec)
        .await?;
        copy.acquisition = row.map(
            |(
                id,
                copy_id,
                acquired_at,
                price_minor,
                currency,
                shipping_minor,
                tax_minor,
                store,
                acquisition_type,
                receipt_path,
            )| AcquisitionDto {
                id,
                copy_id,
                acquired_at,
                price_minor,
                currency,
                shipping_minor,
                tax_minor,
                store,
                acquisition_type,
                receipt_path,
            },
        );
    }
    Ok(())
}

pub async fn get_copy<'a, E>(exec: E, id: &str) -> AppResult<Option<CopyDto>>
where
    E: sqlx::Executor<'a, Database = Sqlite> + Copy,
{
    let row: Option<CopyRow> = sqlx::query_as(&format!(
        "SELECT {COPY_COLUMNS} FROM copies c LEFT JOIN locations l ON l.id = c.location_id WHERE c.id = ?1"
    ))
    .bind(id)
    .fetch_optional(exec)
    .await?;
    let mut copies: Vec<CopyDto> = row.into_iter().map(row_to_dto).collect();
    attach_acquisitions(exec, &mut copies).await?;
    Ok(copies.into_iter().next())
}

pub async fn list_copies_for_edition(
    pool: &SqlitePool,
    edition_id: &str,
) -> AppResult<Vec<CopyDto>> {
    let rows: Vec<CopyRow> = sqlx::query_as(&format!(
        "SELECT {COPY_COLUMNS} FROM copies c LEFT JOIN locations l ON l.id = c.location_id \
         WHERE c.edition_id = ?1 ORDER BY c.created_at"
    ))
    .bind(edition_id)
    .fetch_all(pool)
    .await?;
    let mut copies: Vec<CopyDto> = rows.into_iter().map(row_to_dto).collect();
    attach_acquisitions(pool, &mut copies).await?;
    Ok(copies)
}

/// All non-archived copies across the library (used by future inventory views).
#[allow(dead_code)]
pub async fn list_all_copies(pool: &SqlitePool) -> AppResult<Vec<CopyDto>> {
    let rows: Vec<CopyRow> = sqlx::query_as(&format!(
        "SELECT {COPY_COLUMNS} FROM copies c LEFT JOIN locations l ON l.id = c.location_id \
         WHERE c.archived_at IS NULL ORDER BY c.created_at DESC"
    ))
    .fetch_all(pool)
    .await?;
    let mut copies: Vec<CopyDto> = rows.into_iter().map(row_to_dto).collect();
    attach_acquisitions(pool, &mut copies).await?;
    Ok(copies)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_copy(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    edition_id: &str,
    inventory_code: Option<&str>,
    location_id: Option<&str>,
    condition_grade: &str,
    acquisition_type: Option<&str>,
    acquired_at: Option<&str>,
    note: Option<&str>,
    sentimental_score: Option<i64>,
    acquisition: Option<&crate::models::CreateAcquisitionInput>,
) -> AppResult<String> {
    let id = ids::new_id();
    let now = ids::now_iso();
    sqlx::query(
        "INSERT INTO copies (id, edition_id, inventory_code, location_id, condition_grade, \
         ownership_status, acquisition_type, acquired_at, note, sentimental_score, archived_at, \
         created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'owned', ?6, ?7, ?8, ?9, NULL, ?10, ?10)",
    )
    .bind(&id)
    .bind(edition_id)
    .bind(inventory_code.map(str::trim).filter(|s| !s.is_empty()))
    .bind(location_id)
    .bind(condition_grade)
    .bind(acquisition_type)
    .bind(acquired_at)
    .bind(note.map(str::trim).filter(|s| !s.is_empty()))
    .bind(sentimental_score)
    .bind(&now)
    .execute(&mut **tx)
    .await?;

    if let Some(acq) = acquisition {
        sqlx::query(
            "INSERT INTO acquisitions (id, copy_id, acquired_at, price_minor, currency, \
             shipping_minor, tax_minor, store, acquisition_type, receipt_path, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )
        .bind(ids::new_id())
        .bind(&id)
        .bind(acq.acquired_at.as_deref().or(acquired_at))
        .bind(acq.price_minor)
        .bind(acq.currency.as_deref().map(str::to_uppercase))
        .bind(acq.shipping_minor)
        .bind(acq.tax_minor)
        .bind(
            acq.store
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty()),
        )
        .bind(acq.acquisition_type.as_deref().or(acquisition_type))
        .bind(acq.receipt_path.as_deref().filter(|s| !s.is_empty()))
        .bind(ids::now_iso())
        .execute(&mut **tx)
        .await?;
    }

    Ok(id)
}

pub async fn update_copy(
    pool: &SqlitePool,
    id: &str,
    input: &crate::models::UpdateCopyInput,
) -> AppResult<bool> {
    let result = sqlx::query(
        "UPDATE copies SET location_id = ?2, condition_grade = ?3, note = ?4, \
         sentimental_score = ?5, updated_at = ?6 WHERE id = ?1 AND archived_at IS NULL",
    )
    .bind(id)
    .bind(&input.location_id)
    .bind(&input.condition_grade)
    .bind(
        input
            .note
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(input.sentimental_score)
    .bind(ids::now_iso())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Soft-delete: keep history, hide from active inventory (docs/database.md).
pub async fn archive_copy(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let result = sqlx::query(
        "UPDATE copies SET archived_at = ?2, updated_at = ?2 WHERE id = ?1 AND archived_at IS NULL",
    )
    .bind(id)
    .bind(ids::now_iso())
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
