//! Versioned, lossless library interchange. Table/column identifiers come only
//! from the installed schema; imported values are always bound parameters.
use crate::error::{AppError, AppResult};
use crate::sqlite_db as sqlx;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Column, Row, SqlitePool, TypeInfo, ValueRef};
use std::collections::BTreeMap;

pub const TABLES: &[&str] = &[
    "settings",
    "authors",
    "works",
    "work_authors",
    "editions",
    "market_comparables",
    "locations",
    "copies",
    "acquisitions",
    "tags",
    "work_tags",
    "reading_sessions",
    "reading_progress_events",
    "contacts",
    "loans",
    "valuation_records",
    "sale_listings",
    "disposals",
    "wishlist",
    "activity_events",
    "metadata_cache",
    "import_jobs",
    "attachments",
];

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub format_version: u32,
    pub tables: BTreeMap<String, Vec<BTreeMap<String, Value>>>,
}

pub async fn export(pool: &SqlitePool) -> AppResult<Snapshot> {
    let mut tx = pool.begin().await?;
    let mut tables = BTreeMap::new();
    for &table in TABLES {
        let rows = sqlx::query(&format!("SELECT * FROM \"{table}\""))
            .fetch_all(&mut *tx)
            .await?;
        let mut records = Vec::new();
        for row in rows {
            let mut record = BTreeMap::new();
            for (i, col) in row.columns().iter().enumerate() {
                let raw = row.try_get_raw(i)?;
                let value = if raw.is_null() {
                    Value::Null
                } else if raw.type_info().name() == "INTEGER" {
                    Value::from(row.try_get::<i64, _>(i)?)
                } else {
                    Value::from(row.try_get::<String, _>(i)?)
                };
                record.insert(col.name().into(), value);
            }
            // Profile identity belongs to this installation, not the portable library.
            if table != "settings"
                || record.get("key").and_then(Value::as_str) != Some("_active_profile")
            {
                records.push(record);
            }
        }
        tables.insert(table.into(), records);
    }
    tx.commit().await?;
    Ok(Snapshot {
        format_version: 3,
        tables,
    })
}

/// Replace in a transaction on the live database. WAL and pooled connections
/// remain valid; readers observe either the old library or the committed one.
pub async fn apply(pool: &SqlitePool, snapshot: &Snapshot, replace: bool) -> AppResult<()> {
    apply_inner(pool, snapshot, replace, true).await
}

/// Import staging copies already persisted local data, including legacy values
/// that predate today's validators. This exception cannot be used for incoming
/// files: the destination is always a new in-memory database created here.
pub(crate) async fn stage_local(pool: &SqlitePool) -> AppResult<(SqlitePool, Snapshot)> {
    let baseline = export(pool).await?;
    let staged = crate::db::connect_memory().await?;
    if let Err(error) = apply_inner(&staged, &baseline, true, false).await {
        staged.close().await;
        return Err(error);
    }
    Ok((staged, baseline))
}

/// Only new or changed staged records cross the validation boundary. Unrelated
/// legacy records are neither rewritten nor revalidated during a CSV/array import.
pub(crate) async fn merge_staged(
    pool: &SqlitePool,
    staged: &SqlitePool,
    baseline: &Snapshot,
) -> AppResult<()> {
    let mut delta = export(staged).await?;
    for (table, records) in &mut delta.tables {
        let existing = baseline.tables[table]
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<std::collections::HashSet<_>, _>>()?;
        records.retain(|record| !existing.contains(&serde_json::to_string(record).unwrap()));
    }
    apply(pool, &delta, false).await
}

async fn apply_inner(
    pool: &SqlitePool,
    snapshot: &Snapshot,
    replace: bool,
    validate_incoming: bool,
) -> AppResult<()> {
    if ![1, 2, 3].contains(&snapshot.format_version)
        || snapshot
            .tables
            .keys()
            .any(|t| !TABLES.contains(&t.as_str()))
    {
        return Err(AppError::Validation(
            "unsupported library snapshot format".into(),
        ));
    }
    if replace
        && TABLES.iter().any(|t| {
            !snapshot.tables.contains_key(*t)
                && !(snapshot.format_version < 3 && *t == "market_comparables")
        })
    {
        return Err(AppError::Validation("incomplete library snapshot".into()));
    }
    let mut tx = pool.begin().await?;
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *tx)
        .await?;
    let active_profile: Option<String> = if replace {
        sqlx::query_scalar("SELECT value_json FROM settings WHERE key='_active_profile'")
            .fetch_optional(&mut *tx)
            .await?
    } else {
        None
    };
    if replace {
        for &table in TABLES.iter().rev() {
            sqlx::query(&format!("DELETE FROM \"{table}\""))
                .execute(&mut *tx)
                .await?;
        }
    }
    for &table in TABLES {
        let columns = sqlx::query(&format!("PRAGMA table_info(\"{table}\")"))
            .fetch_all(&mut *tx)
            .await?;
        let allowed: Vec<String> = columns.iter().map(|r| r.get("name")).collect();
        for original_record in snapshot.tables.get(table).into_iter().flatten() {
            let mut upgraded_record;
            let record = if snapshot.format_version == 1 {
                upgraded_record = original_record.clone();
                crate::money::upgrade_legacy_record(table, &mut upgraded_record)
                    .map_err(AppError::Validation)?;
                &upgraded_record
            } else {
                original_record
            };
            if record.is_empty() || record.keys().any(|c| !allowed.contains(c)) {
                return Err(AppError::Validation(format!("invalid columns in {table}")));
            }
            if validate_incoming {
                validate_record(table, record)?;
            }
            for (key, value) in record {
                if validate_incoming && key == "currency" {
                    if let Some(code) = value.as_str() {
                        crate::money::validate_currency(code).map_err(AppError::Validation)?;
                    }
                }
                if validate_incoming
                    && key.ends_with("_path")
                    && value.as_str().is_some_and(|p| {
                        std::path::Path::new(p).is_absolute()
                            || p.split(['/', '\\']).any(|v| v == "..")
                    })
                {
                    return Err(AppError::Validation(
                        "snapshot paths must be relative to app data".into(),
                    ));
                }
                if !value.is_null() && !value.is_string() && value.as_i64().is_none() {
                    return Err(AppError::Validation(format!(
                        "invalid value in {table}.{key}"
                    )));
                }
            }
            let names = record
                .keys()
                .map(|k| format!("\"{k}\""))
                .collect::<Vec<_>>()
                .join(",");
            let placeholders = vec!["?"; record.len()].join(",");
            // Conflicting IDs are never silently overwritten during a merge.
            let conflict = if replace {
                ""
            } else {
                " ON CONFLICT DO NOTHING"
            };
            let sql =
                format!("INSERT INTO \"{table}\" ({names}) VALUES ({placeholders}){conflict}");
            let mut query = sqlx::query(&sql);
            for value in record.values() {
                query = match value {
                    Value::Null => query.bind(None::<String>),
                    Value::String(s) => query.bind(s),
                    _ => query.bind(value.as_i64().expect("validated integer")),
                };
            }
            let inserted = query.execute(&mut *tx).await?;
            if inserted.rows_affected() == 0 && !replace {
                // Existing identical records are idempotent. A differing record
                // is a conflict, including unique ISBN/name collisions.
                let mut check =
                    sqlx::QueryBuilder::new(format!("SELECT count(*) FROM \"{table}\" WHERE "));
                let mut parts = check.separated(" AND ");
                for (key, value) in record {
                    parts.push(format!("\"{key}\" IS "));
                    match value {
                        Value::Null => {
                            parts.push_bind_unseparated(None::<String>);
                        }
                        Value::String(v) => {
                            parts.push_bind_unseparated(v);
                        }
                        _ => {
                            parts.push_bind_unseparated(value.as_i64().unwrap());
                        }
                    }
                }
                let (count,): (i64,) = check.build_query_as().fetch_one(&mut *tx).await?;
                if count == 0 {
                    return Err(AppError::Conflict(format!(
                        "conflicting record in {table}; import into a separate library"
                    )));
                }
            }
        }
    }
    if replace
        && !snapshot
            .tables
            .get("settings")
            .into_iter()
            .flatten()
            .any(|r| r.get("key").and_then(Value::as_str) == Some("_active_profile"))
    {
        if let Some(active) = active_profile {
            sqlx::query("INSERT INTO settings(key,value_json) VALUES ('_active_profile',?)")
                .bind(active)
                .execute(&mut *tx)
                .await?;
        }
    }
    let invalid = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut *tx)
        .await?;
    if !invalid.is_empty() {
        return Err(AppError::Validation(
            "library contains invalid references".into(),
        ));
    }
    let mismatched: (i64,) = sqlx::query_as("SELECT count(*) FROM reading_sessions r JOIN editions e ON e.id=r.edition_id WHERE e.work_id<>r.work_id").fetch_one(&mut *tx).await?;
    if mismatched.0 != 0 {
        return Err(AppError::Validation(
            "reading edition belongs to another work".into(),
        ));
    }
    sqlx::query("DELETE FROM works_fts")
        .execute(&mut *tx)
        .await?;
    let ids: Vec<(String,)> = sqlx::query_as("SELECT id FROM works")
        .fetch_all(&mut *tx)
        .await?;
    for (id,) in ids {
        super::works::sync_fts(&mut tx, &id).await?;
    }
    tx.commit().await?;
    Ok(())
}

fn validate_record(table: &str, record: &BTreeMap<String, Value>) -> AppResult<()> {
    let text = |key: &str| record.get(key).and_then(Value::as_str);
    let number = |key: &str| record.get(key).and_then(Value::as_i64);
    let invalid = |message: String| AppError::Validation(format!("{table}: {message}"));
    for (key, value) in record {
        if (key.ends_with("_at") || key == "date") && key != "published_at" {
            crate::models::validate_date(value.as_str()).map_err(invalid)?;
        }
        if key.ends_with("_minor") && !value.is_null() {
            if value
                .as_i64()
                .is_none_or(|v| !(0..=9_007_199_254_740_991).contains(&v))
            {
                return Err(invalid("amount must be a nonnegative safe integer".into()));
            }
            if text("currency").is_none() {
                return Err(invalid("currency required with an amount".into()));
            }
        }
    }
    for key in ["title", "name", "display_name"] {
        if text(key).is_some_and(|v| v.trim().is_empty()) {
            return Err(invalid(format!("{key} cannot be blank")));
        }
    }
    if table == "market_comparables" {
        let url = text("listing_url").ok_or_else(|| invalid("listing URL required".into()))?;
        let canonical = crate::domain::market::olx_url(url, true)?;
        if canonical != url
            || !matches!(number("included"), Some(0 | 1))
            || text("condition_note").is_some_and(|v| v.chars().count() > 2000)
            || text("observed_at").is_none_or(|v| v.len() != 10)
            || text("currency").is_none_or(|v| v != v.to_ascii_uppercase())
        {
            return Err(invalid("invalid comparison".into()));
        }
    }
    if table == "editions" {
        crate::models::validate_publication(text("published_at"), number("page_count"))
            .map_err(invalid)?;
        let mut canonical = None;
        for key in ["isbn10", "isbn13"] {
            if let Some(value) = text(key).filter(|v| !v.is_empty()) {
                let isbn =
                    crate::isbn::parse(value).ok_or_else(|| invalid("invalid ISBN".into()))?;
                let normalized = isbn.isbn13;
                if canonical.as_ref().is_some_and(|v| v != &normalized) {
                    return Err(invalid("ISBN identities disagree".into()));
                }
                canonical = Some(normalized);
            }
        }
    }
    if table == "reading_sessions"
        && text("started_at")
            .zip(text("finished_at"))
            .is_some_and(|(a, b)| a.get(..10) > b.get(..10))
    {
        return Err(invalid("finished date precedes start".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn round_trip_preserves_history_and_rolls_back_bad_references() {
        let source = crate::db::connect_memory().await.unwrap();
        sqlx::query("INSERT INTO authors VALUES ('a','Last, First',NULL,'2026-01-01')")
            .execute(&source)
            .await
            .unwrap();
        let snapshot = export(&source).await.unwrap();
        let target = crate::db::connect_memory().await.unwrap();
        apply(&target, &snapshot, true).await.unwrap();
        assert_eq!(
            serde_json::to_value(export(&target).await.unwrap()).unwrap(),
            serde_json::to_value(&snapshot).unwrap()
        );
        let mut invalid = snapshot;
        invalid
            .tables
            .get_mut("work_authors")
            .unwrap()
            .push(BTreeMap::from([
                ("work_id".into(), Value::from("missing")),
                ("author_id".into(), Value::from("a")),
            ]));
        assert!(apply(&target, &invalid, true).await.is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM authors")
                .fetch_one(&target)
                .await
                .unwrap(),
            1
        );
    }
}
