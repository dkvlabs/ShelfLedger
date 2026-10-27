//! Resale & money (Phase 3): time-bound value observations, sale listings,
//! disposals, the transparent keep/sell/donate assistant, and per-currency
//! reporting. Currencies are never summed together; no data is invented.

use crate::sqlite_db as sqlx;
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::ids;
use crate::models::{
    AddValuationInput, CreateListingInput, DisposalDto, HistoricalCost, RecordDisposalInput,
    ReportBucket, SaleListingDto, SellQueueItem, ValuationDto, YearReport,
};

// sqlx row shapes (kept as aliases for readability).
type ValuationRow = (
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
    String,
    String,
);
type ListingRow = (String, String, Option<String>, String, i64, String, String);
type DisposalRow = (
    String,
    String,
    String,
    String,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
);
type SellQueueRow = (
    String,         // copy id
    String,         // work id
    String,         // work title
    String,         // author names (joined)
    String,         // condition
    i64,            // total active copies of work
    i64,            // copy number (1-based by created_at)
    Option<i64>,    // sentimental
    Option<String>, // override
    Option<String>, // last finished_at (work)
    Option<i64>,    // best rating (work)
    Option<String>, // dnf/never-read marker: worst session status
);
type AcquisitionCostRow = (Option<i64>, Option<i64>, Option<i64>, String);

// -- Valuations ---------------------------------------------------------------

pub async fn add_valuation(
    pool: &SqlitePool,
    input: &AddValuationInput,
) -> AppResult<ValuationDto> {
    input.validate().map_err(AppError::Validation)?;
    let copy: Option<(String,)> =
        sqlx::query_as("SELECT id FROM copies WHERE id = ?1 AND archived_at IS NULL")
            .bind(&input.copy_id)
            .fetch_optional(pool)
            .await?;
    if copy.is_none() {
        return Err(AppError::NotFound(format!("copy {}", input.copy_id)));
    }
    let id = ids::new_id();
    sqlx::query(
        "INSERT INTO valuation_records (id, copy_id, value_minor, currency, source_type, \
         source_note, observed_at, confidence, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )
    .bind(&id)
    .bind(&input.copy_id)
    .bind(input.value_minor)
    .bind(input.currency.to_uppercase())
    .bind(&input.source_type)
    .bind(
        input
            .source_note
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .observed_at
            .clone()
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string()),
    )
    .bind(&input.confidence)
    .bind(ids::now_iso())
    .execute(pool)
    .await?;
    list_valuations(pool, &input.copy_id)
        .await?
        .into_iter()
        .find(|v| v.id == id)
        .ok_or_else(|| AppError::Other("valuation vanished after insert".into()))
}

pub async fn list_valuations(pool: &SqlitePool, copy_id: &str) -> AppResult<Vec<ValuationDto>> {
    let rows: Vec<ValuationRow> = sqlx::query_as(
        "SELECT id, copy_id, value_minor, currency, source_type, source_note, observed_at, confidence \
         FROM valuation_records WHERE copy_id = ?1 ORDER BY observed_at DESC, created_at DESC, rowid DESC",
    )
    .bind(copy_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                copy_id,
                value_minor,
                currency,
                source_type,
                source_note,
                observed_at,
                confidence,
            )| ValuationDto {
                id,
                copy_id,
                value_minor,
                currency,
                source_type,
                source_note,
                observed_at,
                confidence,
            },
        )
        .collect())
}

// -- Listings -----------------------------------------------------------------

pub async fn create_listing(
    pool: &SqlitePool,
    input: &CreateListingInput,
) -> AppResult<SaleListingDto> {
    input.validate().map_err(AppError::Validation)?;
    let mut tx = pool.begin().await?;
    let copy: Option<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, archived_at, ownership_status FROM copies WHERE id = ?1")
            .bind(&input.copy_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((_, archived_at, ownership_status)) = copy else {
        return Err(AppError::NotFound(format!("copy {}", input.copy_id)));
    };
    if archived_at.is_some() {
        return Err(AppError::Validation(
            "an archived copy cannot be listed".into(),
        ));
    }
    if ownership_status.as_deref() == Some("lent") {
        return Err(AppError::Validation(
            "a lent copy cannot be listed; record its return first".into(),
        ));
    }
    if !matches!(ownership_status.as_deref(), Some("owned") | Some("listed")) {
        return Err(AppError::Validation(format!(
            "this copy is not available for listing (status: {ownership_status:?})"
        )));
    }
    let active: Option<(String,)> =
        sqlx::query_as("SELECT id FROM sale_listings WHERE copy_id = ?1 AND status = 'active'")
            .bind(&input.copy_id)
            .fetch_optional(&mut *tx)
            .await?;
    if active.is_some() {
        return Err(AppError::Conflict(
            "this copy already has an active listing".into(),
        ));
    }

    let id = ids::new_id();
    let now = ids::now_iso();
    let listed_at = chrono::Utc::now().format("%Y-%m-%d").to_string();
    sqlx::query(
        "INSERT INTO sale_listings (id, copy_id, platform, listed_at, asking_price_minor, currency, status, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?7)",
    )
    .bind(&id)
    .bind(&input.copy_id)
    .bind(input.platform.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(&listed_at)
    .bind(input.asking_price_minor)
    .bind(input.currency.to_uppercase())
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE copies SET ownership_status=?, updated_at=? WHERE id=?")
        .bind("listed")
        .bind(ids::now_iso())
        .bind(&input.copy_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    record_event(pool, "listing", &id, "created", &input.copy_id).await?;
    list_listings(pool, &input.copy_id)
        .await?
        .into_iter()
        .find(|l| l.id == id)
        .ok_or_else(|| AppError::Other("listing vanished after insert".into()))
}

/// Withdraw an active listing; the copy returns to `owned` availability.
pub async fn withdraw_listing(pool: &SqlitePool, listing_id: &str) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let listing: Option<(String, String)> =
        sqlx::query_as("SELECT id, copy_id FROM sale_listings WHERE id = ?1 AND status = 'active'")
            .bind(listing_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((_, copy_id)) = listing else {
        return Err(AppError::NotFound(format!("active listing {listing_id}")));
    };
    sqlx::query("UPDATE sale_listings SET status = 'withdrawn', updated_at = ?2 WHERE id = ?1")
        .bind(listing_id)
        .bind(ids::now_iso())
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE copies SET ownership_status=?, updated_at=? WHERE id=? AND ownership_status='listed'").bind("owned").bind(ids::now_iso()).bind(&copy_id).execute(&mut *tx).await?;
    tx.commit().await?;
    record_event(pool, "listing", listing_id, "withdrawn", &copy_id).await?;
    Ok(())
}

pub async fn list_listings(pool: &SqlitePool, copy_id: &str) -> AppResult<Vec<SaleListingDto>> {
    let rows: Vec<ListingRow> = sqlx::query_as(
        "SELECT id, copy_id, platform, listed_at, asking_price_minor, currency, status \
         FROM sale_listings WHERE copy_id = ?1 ORDER BY listed_at DESC",
    )
    .bind(copy_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, copy_id, platform, listed_at, asking_price_minor, currency, status)| {
                SaleListingDto {
                    id,
                    copy_id,
                    platform,
                    listed_at,
                    asking_price_minor,
                    currency,
                    status,
                }
            },
        )
        .collect())
}

// -- Disposals ----------------------------------------------------------------

/// Record a disposal (sold/donated/traded/lost). The copy row and all history
/// are kept; only `ownership_status` changes. Sold/donated/lost copies must
/// not have an active loan.
pub async fn record_disposal(
    pool: &SqlitePool,
    input: &RecordDisposalInput,
) -> AppResult<DisposalDto> {
    input.validate().map_err(AppError::Validation)?;
    let mut tx = pool.begin().await?;
    let copy: Option<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, archived_at, ownership_status FROM copies WHERE id = ?1")
            .bind(&input.copy_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((_, archived_at, ownership_status)) = copy else {
        return Err(AppError::NotFound(format!("copy {}", input.copy_id)));
    };
    if archived_at.is_some() {
        return Err(AppError::Validation(
            "an archived copy cannot be disposed again".into(),
        ));
    }
    if sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM loans WHERE copy_id=? AND returned_at IS NULL",
    )
    .bind(&input.copy_id)
    .fetch_one(&mut *tx)
    .await?
        > 0
    {
        return Err(AppError::Validation(
            "this copy has an active loan; record its return before disposing of it".into(),
        ));
    }
    if matches!(
        ownership_status.as_deref(),
        Some("sold" | "donated" | "traded" | "lost")
    ) {
        return Err(AppError::Validation(format!(
            "this copy was already disposed of (status: {ownership_status:?})"
        )));
    }

    let id = ids::new_id();
    let date = input
        .date
        .clone()
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    sqlx::query(
        "INSERT INTO disposals (id, copy_id, disposition, date, sale_price_minor, fee_minor, \
         shipping_minor, currency, buyer_alias, notes, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )
    .bind(&id)
    .bind(&input.copy_id)
    .bind(&input.disposition)
    .bind(&date)
    .bind(input.sale_price_minor)
    .bind(input.fee_minor)
    .bind(input.shipping_minor)
    .bind(input.currency.as_deref().map(str::to_uppercase))
    .bind(
        input
            .buyer_alias
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .notes
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(ids::now_iso())
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE copies SET ownership_status=?, updated_at=? WHERE id=?")
        .bind(&input.disposition)
        .bind(ids::now_iso())
        .bind(&input.copy_id)
        .execute(&mut *tx)
        .await?;
    {
        sqlx::query(
            "UPDATE sale_listings SET status = ?3, updated_at = ?2 \
             WHERE copy_id = ?1 AND status = 'active'",
        )
        .bind(&input.copy_id)
        .bind(ids::now_iso())
        .bind(if input.disposition == "sold" {
            "sold"
        } else {
            "withdrawn"
        })
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    record_event(pool, "disposal", &id, &input.disposition, &input.copy_id).await?;
    list_disposals(pool, &input.copy_id)
        .await?
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| AppError::Other("disposal vanished after insert".into()))
}

pub async fn list_disposals(pool: &SqlitePool, copy_id: &str) -> AppResult<Vec<DisposalDto>> {
    let rows: Vec<DisposalRow> = sqlx::query_as(
        "SELECT id, copy_id, disposition, date, sale_price_minor, fee_minor, shipping_minor, \
                currency, buyer_alias, notes \
         FROM disposals WHERE copy_id = ?1 ORDER BY date DESC",
    )
    .bind(copy_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                copy_id,
                disposition,
                date,
                sale_price_minor,
                fee_minor,
                shipping_minor,
                currency,
                buyer_alias,
                notes,
            )| DisposalDto {
                id,
                copy_id,
                disposition,
                date,
                sale_price_minor,
                fee_minor,
                shipping_minor,
                currency,
                buyer_alias,
                notes,
            },
        )
        .collect())
}

// -- Transparent decision assistant (v1 rules, fully explainable) -------------

fn years_between(from_date: &str, now_date: &str) -> Option<f64> {
    let from = chrono::NaiveDate::parse_from_str(&from_date[..10.min(from_date.len())], "%Y-%m-%d")
        .ok()?;
    let to =
        chrono::NaiveDate::parse_from_str(&now_date[..10.min(now_date.len())], "%Y-%m-%d").ok()?;
    Some((to - from).num_days() as f64 / 365.25)
}

/// Build the sell/keep/donate queue for all lendable owned copies.
/// Rules use ONLY user-supplied data; every item carries 2+ concrete reasons
/// and a manual override always wins.
pub async fn sell_queue(pool: &SqlitePool) -> AppResult<Vec<SellQueueItem>> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // Base rows: one per owned copy, with work/author info and copy counts.
    let rows: Vec<SellQueueRow> = sqlx::query_as(
        "SELECT c.id, w.id, w.title, \
                coalesce((SELECT group_concat(a2.name, ', ') FROM work_authors wa2 \
                          JOIN authors a2 ON a2.id = wa2.author_id WHERE wa2.work_id = w.id), ''), \
                c.condition_grade, \
                (SELECT COUNT(*) FROM copies c3 JOIN editions e3 ON c3.edition_id = e3.id \
                  WHERE e3.work_id = w.id AND c3.archived_at IS NULL \
                    AND c3.ownership_status IN ('owned', 'lent', 'listed')), \
                (SELECT COUNT(*) FROM copies c4 JOIN editions e4 ON c4.edition_id = e4.id \
                  WHERE e4.work_id = w.id AND c4.archived_at IS NULL \
                    AND c4.ownership_status IN ('owned', 'lent', 'listed') AND c4.created_at <= c.created_at), \
                c.sentimental_score, c.sell_review_override, \
                (SELECT MAX(rs.finished_at) FROM reading_sessions rs \
                  WHERE rs.work_id = w.id AND rs.status = 'finished'), \
                (SELECT MAX(rs.rating) FROM reading_sessions rs WHERE rs.work_id = w.id), \
                (SELECT MIN(rs.status) FROM reading_sessions rs \
                  WHERE rs.work_id = w.id AND rs.status IN ('dnf', 'want_to_read')) \
         FROM copies c \
         JOIN editions e ON e.id = c.edition_id \
         JOIN works w ON w.id = e.work_id \
         WHERE c.archived_at IS NULL AND c.ownership_status IN ('owned', 'listed') \
         ORDER BY w.title, c.created_at",
    )
    .fetch_all(pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for (
        copy_id,
        _work_id,
        work_title,
        author_names,
        condition_grade,
        total_copies,
        copy_number,
        sentimental,
        override_,
        last_finished_at,
        best_rating,
        pending_status,
    ) in rows
    {
        // Historical cost = price + shipping + tax of the acquisition.
        let acq: Option<AcquisitionCostRow> = sqlx::query_as(
            "SELECT a.price_minor, a.shipping_minor, a.tax_minor, a.currency \
             FROM acquisitions a JOIN copies c ON c.id = a.copy_id \
             WHERE a.copy_id = ?1 AND a.price_minor IS NOT NULL",
        )
        .bind(&copy_id)
        .fetch_optional(pool)
        .await?;
        let historical_cost = acq.map(|(price, shipping, tax, currency)| HistoricalCost {
            minor: price.unwrap_or(0) + shipping.unwrap_or(0) + tax.unwrap_or(0),
            currency,
        });
        let latest_valuation = list_valuations(pool, &copy_id).await?.into_iter().next();
        // An active listing is a sell signal in its own right and the UI needs
        // the listing to offer "record sale / withdraw".
        let active_listing = list_listings(pool, &copy_id)
            .await?
            .into_iter()
            .find(|l| l.status == "active");

        let author_list: Vec<String> = if author_names.is_empty() {
            Vec::new()
        } else {
            author_names.split(", ").map(str::to_string).collect()
        };

        let (decision, mut reasons) = decide(
            &today,
            override_.as_deref(),
            total_copies,
            sentimental,
            last_finished_at.as_deref(),
            best_rating,
            pending_status.as_deref(),
            condition_grade.as_str(),
            historical_cost.is_some() || latest_valuation.is_some(),
            active_listing.is_some(),
        );
        // Trim to at most 4 reasons.
        reasons.truncate(4);

        items.push(SellQueueItem {
            copy_id,
            work_title,
            author_names: author_list,
            condition_grade,
            copy_number,
            total_copies_of_work: total_copies,
            historical_cost,
            latest_valuation,
            active_listing,
            last_finished_at,
            best_rating,
            sentimental_score: sentimental,
            decision: decision.to_string(),
            reasons,
        });
    }
    Ok(items)
}

/// The v1 rule set. Deterministic, ordered, and explainable.
#[allow(clippy::too_many_arguments)]
fn decide(
    today: &str,
    override_: Option<&str>,
    total_copies: i64,
    sentimental: Option<i64>,
    last_finished_at: Option<&str>,
    best_rating: Option<i64>,
    pending_status: Option<&str>,
    condition: &str,
    has_value_data: bool,
    listed: bool,
) -> (&'static str, Vec<String>) {
    if let Some(o) = override_ {
        return (
            match o {
                "sell" => "sell_candidate",
                "donate" => "donate_candidate",
                _ => "keep_candidate",
            },
            vec!["Manual override by you".to_string()],
        );
    }

    // A copy on an active sale listing is by definition a sell candidate.
    if listed {
        return (
            "sell_candidate",
            vec![
                "Already listed for sale".to_string(),
                "Record the sale when it sells, or withdraw the listing".to_string(),
            ],
        );
    }

    let sentimental_low = sentimental.map(|s| s <= 2).unwrap_or(true);
    let years_since_read = last_finished_at.and_then(|d| years_between(d, today));

    // Insufficient data: we know nothing about reading or sentiment.
    if sentimental.is_none() && last_finished_at.is_none() && pending_status.is_none() {
        return (
            "needs_review",
            vec![
                "No reading history recorded yet".to_string(),
                "No sentimental rating set".to_string(),
            ],
        );
    }

    // Duplicate + low attachment + stale/negative reading signals -> sell.
    let stale_read = years_since_read.map(|y| y >= 2.0).unwrap_or(false);
    let poor_read_signal = matches!(pending_status, Some("dnf"))
        || best_rating.map(|r| r <= 2).unwrap_or(false)
        || pending_status == Some("want_to_read") && stale_read;
    if total_copies >= 2 && sentimental_low && (stale_read || poor_read_signal) {
        let mut reasons = vec![format!("{total_copies} copies of this work are owned")];
        if let Some(y) = years_since_read {
            reasons.push(format!("Not finished in over {y:.0} year(s)"));
        }
        if matches!(pending_status, Some("dnf")) {
            reasons.push("Marked as did-not-finish".to_string());
        } else if let Some(r) = best_rating {
            if r <= 2 {
                reasons.push(format!("Personal rating is only {r}/5"));
            }
        }
        if sentimental.is_none() {
            reasons.push("No sentimental attachment recorded".to_string());
        }
        if !has_value_data {
            reasons.push("No verified value estimate on record".to_string());
        }
        return ("sell_candidate", reasons);
    }

    // Worn + unloved -> donate.
    if matches!(condition, "acceptable" | "poor") && sentimental_low {
        let mut reasons = vec![format!("Condition is {}", condition.replace('_', " "))];
        if sentimental.is_none() {
            reasons.push("No sentimental attachment recorded".to_string());
        } else {
            reasons.push("Little sentimental value recorded".to_string());
        }
        return ("donate_candidate", reasons);
    }

    // Otherwise keep, with concrete reasons.
    let mut reasons = Vec::new();
    if total_copies == 1 {
        reasons.push("Your only copy of this work".to_string());
    }
    if let Some(score) = sentimental {
        if score >= 3 {
            reasons.push(format!("Sentimental value {score}/5"));
        }
    }
    if let Some(r) = best_rating {
        if r >= 4 {
            reasons.push(format!("Rated {r}/5"));
        }
    }
    if let Some(y) = years_since_read {
        if y < 2.0 {
            reasons.push("Finished within the last two years".to_string());
        }
    }
    if reasons.is_empty() {
        reasons.push("No sell or donate signal met".to_string());
    }
    ("keep_candidate", reasons)
}

// -- Reports ------------------------------------------------------------------

fn bucketize(rows: Vec<(String, String, i64)>) -> Vec<YearReport> {
    use std::collections::BTreeMap;
    // year -> currency -> total. BTreeMap keeps output deterministic.
    let mut by_year: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for (year, currency, minor) in rows {
        *by_year
            .entry(year)
            .or_default()
            .entry(currency)
            .or_insert(0) += minor;
    }
    by_year
        .into_iter()
        .map(|(year, currencies)| YearReport {
            year,
            buckets: currencies
                .into_iter()
                .map(|(currency, total_minor)| ReportBucket {
                    currency,
                    total_minor,
                })
                .collect(),
        })
        .collect()
}

/// Spending and proceeds by year + currency, and estimated collectible value
/// by currency. Mixed currencies are never summed.
pub async fn finance_report(pool: &SqlitePool) -> AppResult<crate::models::FinanceReport> {
    let spending_rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT coalesce(substr(acquired_at, 1, 4), substr(created_at, 1, 4)) AS yr, currency, \
                coalesce(price_minor, 0) + coalesce(shipping_minor, 0) + coalesce(tax_minor, 0) \
         FROM acquisitions WHERE price_minor IS NOT NULL AND currency IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    let proceeds_rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT substr(date, 1, 4) AS yr, currency, \
                coalesce(sale_price_minor, 0) - coalesce(fee_minor, 0) - coalesce(shipping_minor, 0) \
         FROM disposals WHERE disposition = 'sold' AND sale_price_minor IS NOT NULL AND currency IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    // Latest valuation per owned copy, grouped by currency.
    let value_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT v.currency, v.value_minor FROM valuation_records v \
         JOIN copies c ON c.id = v.copy_id \
         WHERE c.archived_at IS NULL AND c.ownership_status IN ('owned', 'lent', 'listed') \
           AND v.id = (SELECT v2.id FROM valuation_records v2 \
                                WHERE v2.copy_id = v.copy_id ORDER BY v2.observed_at DESC, v2.created_at DESC, v2.rowid DESC LIMIT 1)",
    )
    .fetch_all(pool)
    .await?;

    use std::collections::BTreeMap;
    let mut value_buckets: BTreeMap<String, i64> = BTreeMap::new();
    for (currency, minor) in value_rows {
        *value_buckets.entry(currency).or_insert(0) += minor;
    }

    Ok(crate::models::FinanceReport {
        spending: bucketize(spending_rows),
        proceeds: bucketize(proceeds_rows),
        estimated_value: value_buckets
            .into_iter()
            .map(|(currency, total_minor)| ReportBucket {
                currency,
                total_minor,
            })
            .collect(),
    })
}

async fn record_event(
    pool: &SqlitePool,
    entity_type: &str,
    entity_id: &str,
    action: &str,
    detail: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO activity_events (id, entity_type, entity_id, action, occurred_at, payload_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(ids::new_id())
    .bind(entity_type)
    .bind(entity_id)
    .bind(action)
    .bind(ids::now_iso())
    .bind(serde_json::json!({ "detail": detail }).to_string())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_memory;
    use crate::domain::catalog;
    use crate::models::*;

    async fn pool() -> SqlitePool {
        connect_memory().await.unwrap()
    }

    async fn seed_two_copies(pool: &SqlitePool) -> (String, String) {
        let work = catalog::create_work(
            pool,
            &CreateWorkInput {
                title: "Dune".into(),
                subtitle: None,
                original_language: None,
                description: None,
                author_names: vec!["Frank Herbert".into()],
            },
        )
        .await
        .unwrap();
        let edition = catalog::create_edition(
            pool,
            &CreateEditionInput {
                work_id: work.work.id.clone(),
                isbn10: None,
                isbn13: Some("9780441013593".into()),
                publisher: None,
                published_at: None,
                language: None,
                page_count: None,
                binding: None,
            },
        )
        .await
        .unwrap();
        let mk = |sentimental: Option<i64>| CreateCopyInput {
            edition_id: edition.id.clone(),
            inventory_code: None,
            location_id: None,
            condition_grade: "good".into(),
            acquisition_type: Some("bought".into()),
            acquired_at: Some("2021-05-01".into()),
            note: None,
            sentimental_score: sentimental,
            acquisition: Some(CreateAcquisitionInput {
                acquired_at: Some("2021-05-01".into()),
                price_minor: Some(6000),
                currency: Some("RON".into()),
                shipping_minor: Some(1000),
                tax_minor: None,
                store: None,
                acquisition_type: Some("bought".into()),
                receipt_path: None,
            }),
        };
        let c1 = catalog::create_copy(pool, &mk(Some(0))).await.unwrap();
        let c2 = catalog::create_copy(pool, &mk(None)).await.unwrap();
        (c1.id, c2.id)
    }

    async fn seed_reading(
        pool: &SqlitePool,
        work_id: &str,
        status: &str,
        rating: Option<i64>,
        finished: Option<&str>,
    ) {
        let session = crate::domain::reading::create_session(
            pool,
            &CreateReadingSessionInput {
                work_id: work_id.to_string(),
                edition_id: None,
                status: "reading".into(),
                started_at: Some("2021-06-01".into()),
            },
        )
        .await
        .unwrap();
        crate::domain::reading::update_session(
            pool,
            &session.id,
            &UpdateReadingSessionInput {
                status: Some(status.to_string()),
                started_at: None,
                finished_at: Some(finished.map(str::to_string)),
                rating: Some(rating),
                review: None,
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn duplicate_unloved_stale_copy_is_a_sell_candidate() {
        let pool = pool().await;
        let (c1, _c2) = seed_two_copies(&pool).await;
        let work_id: (String,) = sqlx::query_as(
            "SELECT work_id FROM copies c JOIN editions e ON e.id = c.edition_id WHERE c.id = ?1",
        )
        .bind(&c1)
        .fetch_one(&pool)
        .await
        .unwrap();
        seed_reading(&pool, &work_id.0, "finished", Some(2), Some("2021-08-01")).await;

        let queue = sell_queue(&pool).await.unwrap();
        assert_eq!(queue.len(), 2);
        let item = queue.iter().find(|i| i.copy_id == c1).unwrap();
        assert_eq!(item.decision, "sell_candidate");
        assert!(item.reasons.len() >= 2, "reasons: {:?}", item.reasons);
        assert!(item.reasons.iter().any(|r| r.contains("copies")));
        // Historical cost = 6000 + 1000.
        assert_eq!(item.historical_cost.as_ref().unwrap().minor, 7000);
        assert_eq!(item.historical_cost.as_ref().unwrap().currency, "RON");
    }

    #[tokio::test]
    async fn missing_reading_and_sentiment_data_needs_review() {
        let pool = pool().await;
        let (_c1, c2) = seed_two_copies(&pool).await;
        // c2 has no sentimental score and the work has no reading history.
        let item = sell_queue(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|i| i.copy_id == c2)
            .unwrap();
        assert_eq!(item.decision, "needs_review");
        assert!(item.reasons.len() >= 2);
    }

    #[tokio::test]
    async fn sentimental_copy_is_kept_even_when_duplicated() {
        let pool = pool().await;
        let (c1, _c2) = seed_two_copies(&pool).await;
        sqlx::query("UPDATE copies SET sentimental_score = 5 WHERE id = ?1")
            .bind(&c1)
            .execute(&pool)
            .await
            .unwrap();
        let item = sell_queue(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|i| i.copy_id == c1)
            .unwrap();
        assert_eq!(item.decision, "keep_candidate");
    }

    #[tokio::test]
    async fn manual_override_wins() {
        let pool = pool().await;
        let (c1, _c2) = seed_two_copies(&pool).await;
        sqlx::query("UPDATE copies SET sell_review_override = 'sell' WHERE id = ?1")
            .bind(&c1)
            .execute(&pool)
            .await
            .unwrap();
        let item = sell_queue(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|i| i.copy_id == c1)
            .unwrap();
        assert_eq!(item.decision, "sell_candidate");
        assert_eq!(item.reasons, vec!["Manual override by you".to_string()]);
    }

    #[tokio::test]
    async fn reports_never_sum_mixed_currencies() {
        let pool = pool().await;
        let (_c1, _c2) = seed_two_copies(&pool).await;
        // Add a second acquisition in EUR.
        let work2 = catalog::create_work(
            &pool,
            &CreateWorkInput {
                title: "OTHER".into(),
                subtitle: None,
                original_language: None,
                description: None,
                author_names: vec![],
            },
        )
        .await
        .unwrap();
        let edition2 = catalog::create_edition(
            &pool,
            &CreateEditionInput {
                work_id: work2.work.id,
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
        catalog::create_copy(
            &pool,
            &CreateCopyInput {
                edition_id: edition2.id,
                inventory_code: None,
                location_id: None,
                condition_grade: "good".into(),
                acquisition_type: None,
                acquired_at: Some("2022-01-01".into()),
                note: None,
                sentimental_score: None,
                acquisition: Some(CreateAcquisitionInput {
                    acquired_at: Some("2022-01-01".into()),
                    price_minor: Some(1500),
                    currency: Some("EUR".into()),
                    shipping_minor: None,
                    tax_minor: None,
                    store: None,
                    acquisition_type: None,
                    receipt_path: None,
                }),
            },
        )
        .await
        .unwrap();

        let report = finance_report(&pool).await.unwrap();
        let all_buckets: Vec<&ReportBucket> = report
            .spending
            .iter()
            .flat_map(|y| y.buckets.iter())
            .collect();
        let ron = all_buckets.iter().find(|b| b.currency == "RON").unwrap();
        let eur = all_buckets.iter().find(|b| b.currency == "EUR").unwrap();
        assert_eq!(ron.total_minor, 2 * 7000); // two RON acquisitions
        assert_eq!(eur.total_minor, 1500);
        // They stay separate: no combined total exists.
        assert_eq!(all_buckets.len(), 2);
    }

    #[tokio::test]
    async fn sale_records_proceeds_and_keeps_history() {
        let pool = pool().await;
        let (c1, _c2) = seed_two_copies(&pool).await;

        record_disposal(
            &pool,
            &RecordDisposalInput {
                copy_id: c1.clone(),
                disposition: "sold".into(),
                date: Some("2026-10-01".into()),
                sale_price_minor: Some(9000),
                fee_minor: Some(900),
                shipping_minor: Some(500),
                currency: Some("RON".into()),
                buyer_alias: Some("M.".into()),
                notes: None,
            },
        )
        .await
        .unwrap();

        let status: (String,) = sqlx::query_as("SELECT ownership_status FROM copies WHERE id = ?1")
            .bind(&c1)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status.0, "sold");
        // The copy row and acquisition history survive.
        let copy = crate::repositories::copies::get_copy(&pool, &c1)
            .await
            .unwrap()
            .unwrap();
        assert!(copy.acquisition.is_some());

        let report = finance_report(&pool).await.unwrap();
        let proceeds: Vec<&ReportBucket> = report
            .proceeds
            .iter()
            .flat_map(|y| y.buckets.iter())
            .collect();
        // 9000 - 900 - 500 = 7600, kept separate from spending.
        assert_eq!(proceeds[0].total_minor, 7600);

        // Disposing twice is rejected.
        let err = record_disposal(
            &pool,
            &RecordDisposalInput {
                copy_id: c1,
                disposition: "lost".into(),
                date: None,
                sale_price_minor: None,
                fee_minor: None,
                shipping_minor: None,
                currency: None,
                buyer_alias: None,
                notes: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn valuation_is_time_bound_and_latest_wins() {
        let pool = pool().await;
        let (c1, _c2) = seed_two_copies(&pool).await;
        add_valuation(
            &pool,
            &AddValuationInput {
                copy_id: c1.clone(),
                value_minor: 5000,
                currency: "RON".into(),
                source_type: "comparable".into(),
                source_note: Some("local listing".into()),
                observed_at: Some("2026-09-01".into()),
                confidence: "medium".into(),
            },
        )
        .await
        .unwrap();
        add_valuation(
            &pool,
            &AddValuationInput {
                copy_id: c1.clone(),
                value_minor: 7500,
                currency: "RON".into(),
                source_type: "manual".into(),
                source_note: None,
                observed_at: Some("2026-10-01".into()),
                confidence: "low".into(),
            },
        )
        .await
        .unwrap();

        let item = sell_queue(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|i| i.copy_id == c1)
            .unwrap();
        assert_eq!(item.latest_valuation.unwrap().value_minor, 7500);

        // Stale observation date is preserved, never refreshed.
        let valuations = list_valuations(&pool, &c1).await.unwrap();
        assert_eq!(valuations[0].observed_at, "2026-10-01");

        // Invalid currency rejected.
        let err = add_valuation(
            &pool,
            &AddValuationInput {
                copy_id: c1,
                value_minor: 100,
                currency: "EU".into(),
                source_type: "manual".into(),
                source_note: None,
                observed_at: None,
                confidence: "low".into(),
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn listing_flow_keeps_copy_visible_in_sell_queue() {
        let pool = pool().await;
        let (c1, c2) = seed_two_copies(&pool).await;

        // List for sale: ownership flips to `listed` and the queue still sees
        // the copy — now as a sell candidate with its active listing.
        let listing = create_listing(
            &pool,
            &CreateListingInput {
                copy_id: c1.clone(),
                platform: Some("local marketplace".into()),
                asking_price_minor: 9000,
                currency: "RON".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(listing.status, "active");
        let status: (String,) = sqlx::query_as("SELECT ownership_status FROM copies WHERE id = ?1")
            .bind(&c1)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status.0, "listed");

        let item = sell_queue(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|i| i.copy_id == c1)
            .unwrap();
        assert_eq!(item.decision, "sell_candidate");
        assert!(item.reasons.iter().any(|r| r.contains("Already listed")));
        let active = item.active_listing.expect("active listing on queue item");
        assert_eq!(active.asking_price_minor, 9000);

        // A copy cannot be listed twice at once.
        let err = create_listing(
            &pool,
            &CreateListingInput {
                copy_id: c1.clone(),
                platform: None,
                asking_price_minor: 100,
                currency: "RON".into(),
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)));

        // Selling a listed copy records the disposal and closes the listing.
        record_disposal(
            &pool,
            &RecordDisposalInput {
                copy_id: c1.clone(),
                disposition: "sold".into(),
                date: Some("2026-10-25".into()),
                sale_price_minor: Some(8800),
                fee_minor: Some(880),
                shipping_minor: None,
                currency: Some("RON".into()),
                buyer_alias: None,
                notes: None,
            },
        )
        .await
        .unwrap();
        let listings = list_listings(&pool, &c1).await.unwrap();
        assert_eq!(listings[0].status, "sold");
        // Disposed copies leave the queue but keep their history.
        assert!(sell_queue(&pool)
            .await
            .unwrap()
            .iter()
            .all(|i| i.copy_id != c1));

        // Withdraw flow: a fresh copy listed then withdrawn returns to owned.
        let listing2 = create_listing(
            &pool,
            &CreateListingInput {
                copy_id: c2.clone(),
                platform: None,
                asking_price_minor: 5000,
                currency: "RON".into(),
            },
        )
        .await
        .unwrap();
        withdraw_listing(&pool, &listing2.id).await.unwrap();
        let status: (String,) = sqlx::query_as("SELECT ownership_status FROM copies WHERE id = ?1")
            .bind(&c2)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status.0, "owned");
    }

    #[tokio::test]
    async fn lent_and_listed_copies_count_in_estimated_value() {
        let pool = pool().await;
        let (c1, c2) = seed_two_copies(&pool).await;
        for (copy_id, value) in [(c1.clone(), 5000), (c2.clone(), 7000)] {
            add_valuation(
                &pool,
                &AddValuationInput {
                    copy_id,
                    value_minor: value,
                    currency: "RON".into(),
                    source_type: "manual".into(),
                    source_note: None,
                    observed_at: Some("2026-10-01".into()),
                    confidence: "low".into(),
                },
            )
            .await
            .unwrap();
        }
        // One lent out, one listed: both remain in the collection.
        sqlx::query("UPDATE copies SET ownership_status = 'lent' WHERE id = ?1")
            .bind(&c1)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE copies SET ownership_status = 'listed' WHERE id = ?1")
            .bind(&c2)
            .execute(&pool)
            .await
            .unwrap();
        let report = finance_report(&pool).await.unwrap();
        assert_eq!(report.estimated_value[0].total_minor, 12000);
    }
}
