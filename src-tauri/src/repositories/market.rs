use crate::{
    domain::market::{MarketComparable, SaveMarketComparableInput},
    error::{AppError, AppResult},
    ids,
    models::ValuationDto,
    sqlite_db as sqlx,
};
use sqlx::{Row, Sqlite, SqliteConnection, SqlitePool};

pub async fn list<'a, E: sqlx::Executor<'a, Database = Sqlite>>(
    exec: E,
    edition_id: &str,
) -> AppResult<Vec<MarketComparable>> {
    let rows = sqlx::query("SELECT * FROM market_comparables WHERE edition_id=? ORDER BY observed_at DESC, created_at DESC, id")
        .bind(edition_id).fetch_all(exec).await?;
    Ok(rows
        .into_iter()
        .map(|r| MarketComparable {
            id: r.get("id"),
            edition_id: r.get("edition_id"),
            listing_url: r.get("listing_url"),
            asking_minor: r.get("asking_minor"),
            currency: r.get("currency"),
            condition_note: r.get("condition_note"),
            observed_at: r.get("observed_at"),
            included: r.get::<i64, _>("included") == 1,
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}
pub async fn save(
    pool: &SqlitePool,
    input: &SaveMarketComparableInput,
    url: &str,
) -> AppResult<()> {
    sqlx::query("INSERT INTO market_comparables(id,edition_id,listing_url,asking_minor,currency,condition_note,observed_at,included,created_at,updated_at) VALUES(?,?,?,?,?,?,?,1,?,?) ON CONFLICT(edition_id,listing_url) DO UPDATE SET asking_minor=excluded.asking_minor,currency=excluded.currency,condition_note=excluded.condition_note,observed_at=excluded.observed_at,included=1,updated_at=excluded.updated_at")
        .bind(ids::new_id()).bind(&input.edition_id).bind(url).bind(input.asking_minor).bind(input.currency.to_ascii_uppercase())
        .bind(input.condition_note.as_deref().map(str::trim).filter(|s|!s.is_empty())).bind(&input.observed_at).bind(ids::now_iso()).bind(ids::now_iso())
        .execute(pool).await?;
    Ok(())
}
pub async fn set_included(pool: &SqlitePool, id: &str, included: bool) -> AppResult<()> {
    let result = sqlx::query("UPDATE market_comparables SET included=?,updated_at=? WHERE id=?")
        .bind(i64::from(included))
        .bind(ids::now_iso())
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("comparison".into()));
    }
    Ok(())
}
pub async fn check_copy(conn: &mut SqliteConnection, copy: &str, edition: &str) -> AppResult<()> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM copies WHERE id=? AND edition_id=? AND archived_at IS NULL AND ownership_status IN ('owned','lent','listed')")
        .bind(copy).bind(edition).fetch_one(conn).await?;
    if count != 1 {
        return Err(AppError::Validation(
            "choose an active owned copy of this edition".into(),
        ));
    }
    Ok(())
}
pub async fn insert_estimate(conn: &mut SqliteConnection, v: &ValuationDto) -> AppResult<()> {
    sqlx::query("INSERT INTO valuation_records(id,copy_id,value_minor,currency,source_type,source_note,observed_at,confidence,created_at) VALUES(?,?,?,?,?,?,?,?,?)")
        .bind(&v.id).bind(&v.copy_id).bind(v.value_minor).bind(&v.currency).bind(&v.source_type).bind(&v.source_note).bind(&v.observed_at).bind(&v.confidence).bind(ids::now_iso()).execute(conn).await?;
    Ok(())
}
