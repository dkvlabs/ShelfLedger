//! Dashboard summary (Phase 2/3): counts, currently reading, loans, and
//! per-currency money buckets.

use crate::sqlite_db as sqlx;
use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::models::{DashboardDto, ReadingNow};

// session id, work id, work title, started_at, latest page.
type ReadingNowRow = (String, String, String, Option<String>, Option<i64>);

pub async fn get_dashboard(pool: &SqlitePool, local_today: &str) -> AppResult<DashboardDto> {
    let (work_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM works")
        .fetch_one(pool)
        .await?;

    let (owned_copy_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM copies WHERE archived_at IS NULL AND ownership_status IN ('owned', 'lent', 'listed')",
    )
    .fetch_one(pool)
    .await?;

    // Unread backlog: works with an owned copy but no finished session.
    let (unread_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT w.id) FROM works w \
         JOIN editions e ON e.work_id = w.id \
         JOIN copies c ON c.edition_id = e.id \
         WHERE c.archived_at IS NULL AND c.ownership_status IN ('owned', 'lent', 'listed') \
           AND NOT EXISTS (SELECT 1 FROM reading_sessions rs \
                           WHERE rs.work_id = w.id AND rs.status = 'finished')",
    )
    .fetch_one(pool)
    .await?;

    let reading_rows: Vec<ReadingNowRow> = sqlx::query_as(
        "SELECT rs.id, rs.work_id, w.title, rs.started_at, \
                (SELECT MAX(p.page) FROM reading_progress_events p WHERE p.reading_session_id = rs.id) \
         FROM reading_sessions rs JOIN works w ON w.id = rs.work_id \
         WHERE rs.status = 'reading' ORDER BY rs.updated_at DESC",
    )
    .fetch_all(pool)
    .await?;
    let currently_reading = reading_rows
        .into_iter()
        .map(
            |(session_id, work_id, work_title, started_at, latest_page)| ReadingNow {
                session_id,
                work_id,
                work_title,
                started_at,
                latest_page,
            },
        )
        .collect();

    let (active_loans,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM loans WHERE returned_at IS NULL")
            .fetch_one(pool)
            .await?;
    let (overdue_loans,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM loans WHERE returned_at IS NULL AND due_at IS NOT NULL AND due_at < ?1",
    )
    .bind(local_today)
    .fetch_one(pool)
    .await?;

    let report = super::finance::finance_report(pool).await?;
    fn totals(years: &[crate::models::YearReport]) -> Vec<crate::models::ReportBucket> {
        let mut totals = std::collections::BTreeMap::new();
        for year in years {
            for bucket in &year.buckets {
                *totals.entry(bucket.currency.clone()).or_insert(0) += bucket.total_minor;
            }
        }
        totals
            .into_iter()
            .map(|(currency, total_minor)| crate::models::ReportBucket {
                currency,
                total_minor,
            })
            .collect()
    }
    let spending_by_currency = totals(&report.spending);
    let proceeds_by_currency = totals(&report.proceeds);
    let estimated_value_by_currency = report.estimated_value.clone();

    Ok(DashboardDto {
        work_count,
        owned_copy_count,
        unread_count,
        currently_reading,
        active_loans,
        overdue_loans,
        spending_by_currency,
        estimated_value_by_currency,
        proceeds_by_currency,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_memory;
    use crate::domain::{catalog, reading};
    use crate::models::*;

    #[tokio::test]
    async fn dashboard_counts_and_buckets() {
        let pool = connect_memory().await.unwrap();
        let work = catalog::create_work(
            &pool,
            &CreateWorkInput {
                title: "T".into(),
                subtitle: None,
                original_language: None,
                description: None,
                author_names: vec!["A".into()],
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
        catalog::create_copy(
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
                    price_minor: Some(5000),
                    currency: Some("RON".into()),
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

        // Unread at first (no finished session).
        let dash = get_dashboard(&pool, "2026-10-24").await.unwrap();
        assert_eq!(dash.work_count, 1);
        assert_eq!(dash.owned_copy_count, 1);
        assert_eq!(dash.unread_count, 1);
        assert_eq!(dash.spending_by_currency[0].total_minor, 5000);

        reading::create_session(
            &pool,
            &CreateReadingSessionInput {
                work_id: work.work.id.clone(),
                edition_id: None,
                status: "reading".into(),
                started_at: None,
            },
        )
        .await
        .unwrap();
        let dash = get_dashboard(&pool, "2026-10-24").await.unwrap();
        assert_eq!(dash.currently_reading.len(), 1);
    }
}
