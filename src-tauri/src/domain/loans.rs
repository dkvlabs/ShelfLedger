//! Lending: contacts, lend/return flows, and the one-active-loan-per-copy
//! invariant (enforced here AND by a partial unique index in the schema).
//! Contacts stay local; nothing is ever sent anywhere automatically.

use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::ids;
use crate::models::{
    ContactDto, CreateContactInput, LendCopyInput, LoanDto, ReturnCopyInput, UpdateContactInput,
};

const LOAN_COLUMNS: &str = "l.id, l.copy_id, l.contact_id, c.display_name, w.title, \
     coalesce(cp.inventory_code, e.isbn13, e.id), l.lent_at, l.due_at, l.returned_at, l.notes";

type LoanRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn row_to_dto(local_today: &str, due_at: &Option<String>, base: LoanRow) -> LoanDto {
    let (
        id,
        copy_id,
        contact_id,
        contact_name,
        work_title,
        edition_label,
        lent_at,
        due_at_value,
        returned_at,
        notes,
    ) = base;
    let overdue =
        returned_at.is_none() && due_at.as_deref().map(|d| d < local_today).unwrap_or(false);
    LoanDto {
        id,
        copy_id,
        contact_id,
        contact_name,
        work_title,
        edition_label,
        lent_at,
        due_at: due_at_value,
        returned_at,
        notes,
        overdue,
    }
}

enum LoanFilter {
    All,
    Active,
    Overdue,
    ById(String),
}

async fn query_loans(
    pool: &SqlitePool,
    filter: LoanFilter,
    local_today: &str,
) -> AppResult<Vec<LoanDto>> {
    let (where_clause, bind_id) = match &filter {
        LoanFilter::All => ("", None),
        LoanFilter::Active => ("WHERE l.returned_at IS NULL", None),
        LoanFilter::Overdue => (
            "WHERE l.returned_at IS NULL AND l.due_at IS NOT NULL AND l.due_at < ?1",
            Some(local_today.to_string()),
        ),
        LoanFilter::ById(id) => ("WHERE l.id = ?1", Some(id.clone())),
    };
    let sql = format!(
        "SELECT {LOAN_COLUMNS} FROM loans l \
         JOIN contacts c ON c.id = l.contact_id \
         JOIN copies cp ON cp.id = l.copy_id \
         JOIN editions e ON e.id = cp.edition_id \
         JOIN works w ON w.id = e.work_id \
         {where_clause} ORDER BY l.lent_at DESC"
    );
    let mut query = sqlx::query_as::<_, LoanRow>(&sql);
    if let Some(id) = &bind_id {
        query = query.bind(id);
    }
    let rows = query.fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|base| {
            let due_at = base.7.clone();
            row_to_dto(local_today, &due_at, base)
        })
        .collect())
}

/// All loans (history + active). `local_today` is a YYYY-MM-DD string from the
/// user's local clock, supplied by the frontend.
pub async fn list_loans(pool: &SqlitePool, local_today: &str) -> AppResult<Vec<LoanDto>> {
    query_loans(pool, LoanFilter::All, local_today).await
}

/// Active (not yet returned) loans.
pub async fn list_active_loans(pool: &SqlitePool, local_today: &str) -> AppResult<Vec<LoanDto>> {
    query_loans(pool, LoanFilter::Active, local_today).await
}

/// Overdue = active loans past their due date (user's local date).
pub async fn list_overdue_loans(pool: &SqlitePool, local_today: &str) -> AppResult<Vec<LoanDto>> {
    query_loans(pool, LoanFilter::Overdue, local_today).await
}

pub async fn get_loan(pool: &SqlitePool, id: &str, local_today: &str) -> AppResult<LoanDto> {
    query_loans(pool, LoanFilter::ById(id.to_string()), local_today)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NotFound(format!("loan {id}")))
}

// -- Contacts ----------------------------------------------------------------

type ContactRow = (String, String, Option<String>, Option<String>, String);

pub async fn list_contacts(pool: &SqlitePool) -> AppResult<Vec<ContactDto>> {
    let rows: Vec<ContactRow> = sqlx::query_as(
        "SELECT id, display_name, contact_info, notes, created_at FROM contacts ORDER BY display_name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, display_name, contact_info, notes, created_at)| ContactDto {
                id,
                display_name,
                contact_info,
                notes,
                created_at,
            },
        )
        .collect())
}

pub async fn create_contact(
    pool: &SqlitePool,
    input: &CreateContactInput,
) -> AppResult<ContactDto> {
    input.validate().map_err(AppError::Validation)?;
    let id = ids::new_id();
    sqlx::query("INSERT INTO contacts (id, display_name, contact_info, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
        .bind(&id)
        .bind(input.display_name.trim())
        .bind(input.contact_info.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .bind(input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .bind(ids::now_iso())
        .execute(pool)
        .await?;
    list_contacts(pool)
        .await?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Other("contact vanished after insert".into()))
}

/// Rename/correct a contact; loan history keeps pointing at the same person.
pub async fn update_contact(
    pool: &SqlitePool,
    id: &str,
    input: &UpdateContactInput,
) -> AppResult<ContactDto> {
    if let Some(name) = &input.display_name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("contact name is required".into()));
        }
    }
    let result = sqlx::query(
        "UPDATE contacts SET \
           display_name = coalesce(?2, display_name), \
           contact_info = coalesce(?3, contact_info), \
           notes = coalesce(?4, notes) \
         WHERE id = ?1",
    )
    .bind(id)
    .bind(
        input
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    )
    .bind(
        input
            .contact_info
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
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("contact {id}")));
    }
    list_contacts(pool)
        .await?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Other("contact vanished after update".into()))
}

// -- Lend / return -----------------------------------------------------------

async fn active_loan_exists<'a, E>(exec: E, copy_id: &str) -> AppResult<bool>
where
    E: sqlx::Executor<'a, Database = sqlx::Sqlite>,
{
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM loans WHERE copy_id = ?1 AND returned_at IS NULL")
            .bind(copy_id)
            .fetch_optional(exec)
            .await?;
    Ok(row.is_some())
}

pub async fn lend_copy(pool: &SqlitePool, input: &LendCopyInput) -> AppResult<LoanDto> {
    let local_today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // Copy must exist and be lendable: not archived, owned, not already out.
    let copy: Option<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, archived_at, ownership_status FROM copies WHERE id = ?1")
            .bind(&input.copy_id)
            .fetch_optional(pool)
            .await?;
    let Some((_, archived_at, ownership_status)) = copy else {
        return Err(AppError::NotFound(format!("copy {}", input.copy_id)));
    };
    if archived_at.is_some() {
        return Err(AppError::Validation(
            "an archived copy cannot be lent".into(),
        ));
    }
    if active_loan_exists(pool, &input.copy_id).await? {
        return Err(AppError::Conflict(
            "this copy already has an active loan; record its return first".into(),
        ));
    }
    if ownership_status.as_deref() != Some("owned") {
        return Err(AppError::Validation(format!(
            "this copy is not available to lend (status: {ownership_status:?})"
        )));
    }
    let contact: Option<(String,)> = sqlx::query_as("SELECT id FROM contacts WHERE id = ?1")
        .bind(&input.contact_id)
        .fetch_optional(pool)
        .await?;
    if contact.is_none() {
        return Err(AppError::NotFound(format!("contact {}", input.contact_id)));
    }

    let id = ids::new_id();
    let now = ids::now_iso();
    let lent_at = input.lent_at.clone().unwrap_or_else(|| local_today.clone());
    let result = sqlx::query(
        "INSERT INTO loans (id, copy_id, contact_id, lent_at, due_at, returned_at, notes, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?7)",
    )
    .bind(&id)
    .bind(&input.copy_id)
    .bind(&input.contact_id)
    .bind(&lent_at)
    .bind(&input.due_at)
    .bind(input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        // The partial unique index is the hard backstop.
        if crate::error::is_unique_violation_sqlx(&e) {
            AppError::Conflict(
                "this copy already has an active loan; record its return first".into(),
            )
        } else {
            e.into()
        }
    })?;
    if result.rows_affected() == 0 {
        return Err(AppError::Other("loan insert had no effect".into()));
    }
    // Availability flips to lent while the loan is active.
    sqlx::query("UPDATE copies SET ownership_status = 'lent', updated_at = ?2 WHERE id = ?1")
        .bind(&input.copy_id)
        .bind(ids::now_iso())
        .execute(pool)
        .await?;
    record_event(pool, "loan", &id, "lent", &input.copy_id).await?;
    get_loan(pool, &id, &local_today).await
}

pub async fn return_copy(
    pool: &SqlitePool,
    loan_id: &str,
    input: &ReturnCopyInput,
) -> AppResult<LoanDto> {
    let local_today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let loan: Option<(String, String, Option<String>)> =
        sqlx::query_as("SELECT id, copy_id, returned_at FROM loans WHERE id = ?1")
            .bind(loan_id)
            .fetch_optional(pool)
            .await?;
    let Some((_, copy_id, returned_at)) = loan else {
        return Err(AppError::NotFound(format!("loan {loan_id}")));
    };
    if returned_at.is_some() {
        return Err(AppError::Validation(
            "this loan was already returned".into(),
        ));
    }
    if let Some(g) = &input.condition_grade {
        if !crate::models::CONDITION_GRADES.contains(&g.as_str()) {
            return Err(AppError::Validation(format!(
                "unknown condition grade {g:?}"
            )));
        }
    }
    let returned_date = input
        .returned_at
        .clone()
        .unwrap_or_else(|| local_today.clone());
    sqlx::query("UPDATE loans SET returned_at = ?2, notes = coalesce(?3, notes), updated_at = ?4 WHERE id = ?1")
        .bind(loan_id)
        .bind(&returned_date)
        .bind(input.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .bind(ids::now_iso())
        .execute(pool)
        .await?;
    // Availability returns to owned; reading history is untouched.
    sqlx::query("UPDATE copies SET ownership_status = 'owned', updated_at = ?2 WHERE id = ?1")
        .bind(&copy_id)
        .bind(ids::now_iso())
        .execute(pool)
        .await?;
    if let Some(g) = &input.condition_grade {
        sqlx::query("UPDATE copies SET condition_grade = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(&copy_id)
            .bind(g)
            .bind(ids::now_iso())
            .execute(pool)
            .await?;
    }
    record_event(pool, "loan", loan_id, "returned", &copy_id).await?;
    get_loan(pool, loan_id, &local_today).await
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

    async fn seed_copy_with_edition(pool: &SqlitePool) -> (String, String) {
        let work = catalog::create_work(
            pool,
            &CreateWorkInput {
                title: "The Left Hand of Darkness".into(),
                subtitle: None,
                original_language: None,
                description: None,
                author_names: vec!["Ursula K. Le Guin".into()],
            },
        )
        .await
        .unwrap();
        let edition = catalog::create_edition(
            pool,
            &CreateEditionInput {
                work_id: work.work.id.clone(),
                isbn10: None,
                isbn13: Some("9780441478125".into()),
                publisher: None,
                published_at: None,
                language: None,
                page_count: None,
                binding: None,
            },
        )
        .await
        .unwrap();
        let copy = catalog::create_copy(
            pool,
            &CreateCopyInput {
                edition_id: edition.id.clone(),
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
        (copy.id, work.work.id)
    }

    async fn seed_contact(pool: &SqlitePool, name: &str) -> String {
        create_contact(
            pool,
            &CreateContactInput {
                display_name: name.into(),
                contact_info: None,
                notes: None,
            },
        )
        .await
        .unwrap()
        .id
    }

    #[tokio::test]
    async fn double_loan_is_rejected_until_return() {
        let pool = pool().await;
        let (copy_id, _) = seed_copy_with_edition(&pool).await;
        let ana = seed_contact(&pool, "Ana").await;
        let dan = seed_contact(&pool, "Dan").await;

        let loan = lend_copy(
            &pool,
            &LendCopyInput {
                copy_id: copy_id.clone(),
                contact_id: ana,
                lent_at: None,
                due_at: Some("2026-11-01".into()),
                notes: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(loan.contact_name, "Ana");
        assert!(!loan.overdue);

        // Same copy cannot be lent again while active.
        let err = lend_copy(
            &pool,
            &LendCopyInput {
                copy_id: copy_id.clone(),
                contact_id: dan.clone(),
                lent_at: None,
                due_at: None,
                notes: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)), "got {err:?}");

        // Return reopens availability and history is retained.
        let returned = return_copy(
            &pool,
            &loan.id,
            &ReturnCopyInput {
                returned_at: None,
                condition_grade: Some("very_good".into()),
                note: None,
            },
        )
        .await
        .unwrap();
        assert!(returned.returned_at.is_some());
        let copy_status: (String,) =
            sqlx::query_as("SELECT ownership_status FROM copies WHERE id = ?1")
                .bind(&copy_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(copy_status.0, "owned");
        assert_eq!(list_loans(&pool, "2026-10-24").await.unwrap().len(), 1);

        // Now it can be lent again.
        let second = lend_copy(
            &pool,
            &LendCopyInput {
                copy_id,
                contact_id: dan.clone(),
                lent_at: None,
                due_at: None,
                notes: None,
            },
        )
        .await
        .unwrap();
        assert_ne!(second.id, loan.id);
    }

    #[tokio::test]
    async fn overdue_flag_uses_due_date_vs_local_today() {
        let pool = pool().await;
        let (copy_id, _) = seed_copy_with_edition(&pool).await;
        let ana = seed_contact(&pool, "Ana").await;

        lend_copy(
            &pool,
            &LendCopyInput {
                copy_id,
                contact_id: ana,
                lent_at: Some("2026-09-01".into()),
                due_at: Some("2026-09-15".into()),
                notes: None,
            },
        )
        .await
        .unwrap();

        let overdue = list_overdue_loans(&pool, "2026-10-24").await.unwrap();
        assert_eq!(overdue.len(), 1);
        assert!(overdue[0].overdue);
        // Not overdue when checked before the due date.
        assert!(list_overdue_loans(&pool, "2026-09-10")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn loaned_copy_cannot_be_sold_or_relened_before_return() {
        let pool = pool().await;
        let (copy_id, _) = seed_copy_with_edition(&pool).await;
        let ana = seed_contact(&pool, "Ana").await;
        let loan = lend_copy(
            &pool,
            &LendCopyInput {
                copy_id: copy_id.clone(),
                contact_id: ana,
                lent_at: None,
                due_at: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        // Disposal while lent is blocked (finance domain re-checks too).
        let err = crate::domain::finance::record_disposal(
            &pool,
            &RecordDisposalInput {
                copy_id: copy_id.clone(),
                disposition: "sold".into(),
                date: None,
                sale_price_minor: Some(5000),
                fee_minor: None,
                shipping_minor: None,
                currency: Some("RON".into()),
                buyer_alias: None,
                notes: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");

        // Re-lend blocked by ownership status.
        let dan = seed_contact(&pool, "Dan").await;
        let err = lend_copy(
            &pool,
            &LendCopyInput {
                copy_id,
                contact_id: dan,
                lent_at: None,
                due_at: None,
                notes: None,
            },
        )
        .await
        .unwrap_err();
        // Re-lend is blocked by the active-loan invariant (checked first).
        assert!(matches!(err, AppError::Conflict(_)), "got {err:?}");

        return_copy(
            &pool,
            &loan.id,
            &ReturnCopyInput {
                returned_at: None,
                condition_grade: None,
                note: None,
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn contact_rename_survives_in_loan_history() {
        let pool = pool().await;
        let (copy_id, _) = seed_copy_with_edition(&pool).await;
        let ana = seed_contact(&pool, "Ana").await;
        let loan = lend_copy(
            &pool,
            &LendCopyInput {
                copy_id,
                contact_id: ana.clone(),
                lent_at: None,
                due_at: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        let renamed = update_contact(
            &pool,
            &ana,
            &UpdateContactInput {
                display_name: Some("Ana-Maria".into()),
                contact_info: None,
                notes: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(renamed.display_name, "Ana-Maria");

        let loans = list_loans(&pool, "2026-10-24").await.unwrap();
        assert_eq!(loans[0].contact_name, "Ana-Maria");
        assert_eq!(loans[0].id, loan.id);
    }

    #[tokio::test]
    async fn archived_and_missing_copies_cannot_be_lent() {
        let pool = pool().await;
        let (copy_id, _) = seed_copy_with_edition(&pool).await;
        catalog::archive_copy(&pool, &copy_id).await.unwrap();
        let ana = seed_contact(&pool, "Ana").await;
        let err = lend_copy(
            &pool,
            &LendCopyInput {
                copy_id,
                contact_id: ana,
                lent_at: None,
                due_at: None,
                notes: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    /// Plan test matrix (Inventory): two identical editions/copies; one lent
    /// and one sold; archived history intact.
    #[tokio::test]
    async fn two_copies_one_lent_one_sold_history_intact() {
        let pool = pool().await;
        let work = catalog::create_work(
            &pool,
            &CreateWorkInput {
                title: "Duplicata".into(),
                subtitle: None,
                original_language: None,
                description: None,
                author_names: vec!["Author".into()],
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
        let mk = || CreateCopyInput {
            edition_id: edition.id.clone(),
            inventory_code: None,
            location_id: None,
            condition_grade: "good".into(),
            acquisition_type: Some("bought".into()),
            acquired_at: Some("2026-01-01".into()),
            note: None,
            sentimental_score: None,
            acquisition: Some(CreateAcquisitionInput {
                acquired_at: Some("2026-01-01".into()),
                price_minor: Some(3000),
                currency: Some("RON".into()),
                shipping_minor: None,
                tax_minor: None,
                store: None,
                acquisition_type: Some("bought".into()),
                receipt_path: None,
            }),
        };
        let copy_a = catalog::create_copy(&pool, &mk()).await.unwrap();
        let copy_b = catalog::create_copy(&pool, &mk()).await.unwrap();

        // Copy A goes out on loan.
        let ana = seed_contact(&pool, "Ana").await;
        lend_copy(
            &pool,
            &LendCopyInput {
                copy_id: copy_a.id.clone(),
                contact_id: ana,
                lent_at: Some("2026-10-01".into()),
                due_at: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        // Copy B is sold.
        crate::domain::finance::record_disposal(
            &pool,
            &crate::models::RecordDisposalInput {
                copy_id: copy_b.id.clone(),
                disposition: "sold".into(),
                date: Some("2026-10-15".into()),
                sale_price_minor: Some(3500),
                fee_minor: None,
                shipping_minor: None,
                currency: Some("RON".into()),
                buyer_alias: None,
                notes: None,
            },
        )
        .await
        .unwrap();

        // Both states are independent and history stays intact.
        let loans = list_loans(&pool, "2026-10-24").await.unwrap();
        assert_eq!(loans.len(), 1);
        assert!(loans[0].returned_at.is_none());
        let disposals = crate::domain::finance::list_disposals(&pool, &copy_b.id)
            .await
            .unwrap();
        assert_eq!(disposals.len(), 1);
        assert_eq!(disposals[0].sale_price_minor, Some(3500));

        let a = crate::repositories::copies::get_copy(&pool, &copy_a.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(a.ownership_status, "lent");
        // Sold copy keeps its row AND its acquisition history (not deleted).
        let b = crate::repositories::copies::get_copy(&pool, &copy_b.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(b.ownership_status, "sold");
        assert!(b.acquisition.is_some());

        // The sold copy cannot be lent; the lent copy stays out until returned.
        let dan = seed_contact(&pool, "Dan").await;
        let err = lend_copy(
            &pool,
            &LendCopyInput {
                copy_id: copy_b.id.clone(),
                contact_id: dan,
                lent_at: None,
                due_at: None,
                notes: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }
}
