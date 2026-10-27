//! Reading journal: sessions (want to read / reading / finished / paused /
//! DNF), rereads as separate sessions, progress audit trail.
//!
//! Reading state is deliberately separate from ownership/loan state.

use crate::sqlite_db as sqlx;
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::ids;
use crate::models::{
    AddProgressInput, CreateReadingSessionInput, ReadingProgressDto, ReadingSessionDto,
    UpdateReadingSessionInput,
};

const SESSION_COLUMNS: &str =
    "rs.id, rs.work_id, w.title, rs.edition_id, rs.status, rs.started_at, rs.finished_at, \
     rs.rating, rs.review, rs.created_at, rs.updated_at";

type SessionRow = (
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    String,
    String,
);
type ProgressRow = (
    String,
    String,
    String,
    Option<i64>,
    Option<i64>,
    Option<String>,
);

fn row_to_dto(
    (
        id,
        work_id,
        work_title,
        edition_id,
        status,
        started_at,
        finished_at,
        rating,
        review,
        created_at,
        updated_at,
    ): SessionRow,
) -> ReadingSessionDto {
    ReadingSessionDto {
        id,
        work_id,
        work_title,
        edition_id,
        status,
        started_at,
        finished_at,
        rating,
        review,
        created_at,
        updated_at,
        progress_events: Vec::new(),
    }
}

async fn attach_progress<'a, E>(exec: E, sessions: &mut [ReadingSessionDto]) -> AppResult<()>
where
    E: sqlx::Executor<'a, Database = sqlx::Sqlite> + Copy,
{
    for session in sessions.iter_mut() {
        let rows: Vec<ProgressRow> = sqlx::query_as(
            "SELECT id, reading_session_id, occurred_at, page, percent, note \
             FROM reading_progress_events WHERE reading_session_id = ?1 ORDER BY occurred_at",
        )
        .bind(&session.id)
        .fetch_all(exec)
        .await?;
        session.progress_events = rows
            .into_iter()
            .map(
                |(id, reading_session_id, occurred_at, page, percent, note)| ReadingProgressDto {
                    id,
                    reading_session_id,
                    occurred_at,
                    page,
                    percent,
                    note,
                },
            )
            .collect();
    }
    Ok(())
}

async fn get_session_raw<'a, E>(exec: E, id: &str) -> AppResult<Option<ReadingSessionDto>>
where
    E: sqlx::Executor<'a, Database = sqlx::Sqlite> + Copy,
{
    let row: Option<SessionRow> = sqlx::query_as(&format!(
        "SELECT {SESSION_COLUMNS} FROM reading_sessions rs JOIN works w ON w.id = rs.work_id \
         WHERE rs.id = ?1"
    ))
    .bind(id)
    .fetch_optional(exec)
    .await?;
    let mut sessions: Vec<ReadingSessionDto> = row.into_iter().map(row_to_dto).collect();
    attach_progress(exec, &mut sessions).await?;
    Ok(sessions.into_iter().next())
}

pub async fn list_sessions_for_work(
    pool: &SqlitePool,
    work_id: &str,
) -> AppResult<Vec<ReadingSessionDto>> {
    let rows: Vec<SessionRow> = sqlx::query_as(&format!(
        "SELECT {SESSION_COLUMNS} FROM reading_sessions rs JOIN works w ON w.id = rs.work_id \
         WHERE rs.work_id = ?1 ORDER BY rs.created_at DESC"
    ))
    .bind(work_id)
    .fetch_all(pool)
    .await?;
    let mut sessions: Vec<ReadingSessionDto> = rows.into_iter().map(row_to_dto).collect();
    attach_progress(pool, &mut sessions).await?;
    Ok(sessions)
}

/// All sessions for library-level views, newest first. Empty status keeps
/// everything; a filter narrows to one status.
pub async fn list_sessions(
    pool: &SqlitePool,
    status: Option<&str>,
) -> AppResult<Vec<ReadingSessionDto>> {
    let mut sessions: Vec<ReadingSessionDto> = match status {
        Some(s) => {
            let rows: Vec<SessionRow> = sqlx::query_as(&format!(
                "SELECT {SESSION_COLUMNS} FROM reading_sessions rs JOIN works w ON w.id = rs.work_id \
                 WHERE rs.status = ?1 ORDER BY rs.updated_at DESC"
            ))
            .bind(s)
            .fetch_all(pool)
            .await?;
            rows.into_iter().map(row_to_dto).collect()
        }
        None => {
            let rows: Vec<SessionRow> = sqlx::query_as(&format!(
                "SELECT {SESSION_COLUMNS} FROM reading_sessions rs JOIN works w ON w.id = rs.work_id \
                 ORDER BY rs.updated_at DESC"
            ))
            .fetch_all(pool)
            .await?;
            rows.into_iter().map(row_to_dto).collect()
        }
    };
    attach_progress(pool, &mut sessions).await?;
    Ok(sessions)
}

pub async fn create_session(
    pool: &SqlitePool,
    input: &CreateReadingSessionInput,
) -> AppResult<ReadingSessionDto> {
    input.validate().map_err(AppError::Validation)?;
    if crate::repositories::works::get_work(pool, &input.work_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!("work {}", input.work_id)));
    }
    if let Some(edition_id) = &input.edition_id {
        let edition = crate::repositories::editions::get_edition(pool, edition_id)
            .await?
            .ok_or_else(|| AppError::Validation("edition does not exist".into()))?;
        if edition.work_id != input.work_id {
            return Err(AppError::Validation(
                "edition belongs to a different work".into(),
            ));
        }
    }
    // Reading status is per work; multiple sessions model rereads and DNF ->
    // restart, so no uniqueness constraint here by design.
    let id = ids::new_id();
    let now = ids::now_iso();
    let started_at = input
        .started_at
        .clone()
        .or_else(|| (input.status == "reading").then(|| now.clone()));
    sqlx::query(
        "INSERT INTO reading_sessions (id, work_id, edition_id, status, started_at, finished_at, \
         rating, review, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL, ?6, ?6)",
    )
    .bind(&id)
    .bind(&input.work_id)
    .bind(&input.edition_id)
    .bind(&input.status)
    .bind(started_at)
    .bind(&now)
    .execute(pool)
    .await?;
    record_event(pool, "reading_session", &id, "created", &input.status).await?;
    get_session_raw(pool, &id)
        .await?
        .ok_or_else(|| AppError::Other("session vanished after insert".into()))
}

pub async fn update_session(
    pool: &SqlitePool,
    id: &str,
    input: &UpdateReadingSessionInput,
) -> AppResult<ReadingSessionDto> {
    input.validate().map_err(AppError::Validation)?;
    let existing = get_session_raw(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("reading session {id}")))?;

    let status = input.status.clone().unwrap_or(existing.status.clone());
    // finished/paused/dnf close the session; started_at backfills when
    // transitioning to reading without one (never silently rewritten otherwise).
    let started_at = match (&input.started_at, status.as_str()) {
        (Some(s), _) => s.clone(),
        (None, "reading") if existing.started_at.is_none() => Some(ids::now_iso()),
        _ => existing.started_at.clone(),
    };
    let finished_at = match (&input.finished_at, status.as_str()) {
        (Some(f), _) => f.clone(),
        (None, "finished") if existing.finished_at.is_none() => Some(ids::now_iso()),
        _ => existing.finished_at.clone(),
    };

    if let (Some(a), Some(b)) = (&started_at, &finished_at) {
        if a.get(..10) > b.get(..10) {
            return Err(AppError::Validation(
                "finish date precedes start date".into(),
            ));
        }
    }
    sqlx::query(
        "UPDATE reading_sessions SET status = ?2, started_at = ?3, finished_at = ?4, \
         rating = ?5, review = ?6, updated_at = ?7 WHERE id = ?1",
    )
    .bind(id)
    .bind(&status)
    .bind(started_at)
    .bind(finished_at)
    .bind(input.rating.unwrap_or(existing.rating))
    .bind(input.review.clone().unwrap_or(existing.review))
    .bind(ids::now_iso())
    .execute(pool)
    .await?;
    record_event(pool, "reading_session", id, "updated", &status).await?;
    get_session_raw(pool, id)
        .await?
        .ok_or_else(|| AppError::Other("session vanished after update".into()))
}

pub async fn add_progress(
    pool: &SqlitePool,
    session_id: &str,
    input: &AddProgressInput,
) -> AppResult<ReadingSessionDto> {
    input.validate().map_err(AppError::Validation)?;
    if get_session_raw(pool, session_id).await?.is_none() {
        return Err(AppError::NotFound(format!("reading session {session_id}")));
    }
    sqlx::query(
        "INSERT INTO reading_progress_events (id, reading_session_id, occurred_at, page, percent, note) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(ids::new_id())
    .bind(session_id)
    .bind(input.occurred_at.clone().unwrap_or_else(ids::now_iso))
    .bind(input.page)
    .bind(input.percent)
    .bind(input.note.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .execute(pool)
    .await?;
    get_session_raw(pool, session_id)
        .await?
        .ok_or_else(|| AppError::Other("session vanished after progress insert".into()))
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
    use crate::models::CreateWorkInput;

    async fn pool() -> SqlitePool {
        connect_memory().await.unwrap()
    }

    async fn seed_work(pool: &SqlitePool) -> String {
        catalog::create_work(
            pool,
            &CreateWorkInput {
                title: "The Dispossessed".into(),
                subtitle: None,
                original_language: None,
                description: None,
                author_names: vec!["Ursula K. Le Guin".into()],
            },
        )
        .await
        .unwrap()
        .work
        .id
    }

    #[tokio::test]
    async fn rereads_are_separate_sessions() {
        let pool = pool().await;
        let work_id = seed_work(&pool).await;

        let first = create_session(
            &pool,
            &CreateReadingSessionInput {
                work_id: work_id.clone(),
                edition_id: None,
                status: "reading".into(),
                started_at: Some("2024-01-01".into()),
            },
        )
        .await
        .unwrap();
        let first = update_session(
            &pool,
            &first.id,
            &UpdateReadingSessionInput {
                status: Some("finished".into()),
                started_at: None,
                finished_at: Some(Some("2024-02-01".into())),
                rating: Some(Some(5)),
                review: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(first.status, "finished");
        assert_eq!(first.finished_at.as_deref(), Some("2024-02-01"));

        // Second (reread) session is a new row with its own dates.
        let second = create_session(
            &pool,
            &CreateReadingSessionInput {
                work_id: work_id.clone(),
                edition_id: None,
                status: "reading".into(),
                started_at: Some("2026-10-01".into()),
            },
        )
        .await
        .unwrap();
        let sessions = list_sessions_for_work(&pool, &work_id).await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert_ne!(first.id, second.id);
    }

    #[tokio::test]
    async fn dnf_then_restart_preserves_history() {
        let pool = pool().await;
        let work_id = seed_work(&pool).await;

        let attempt = create_session(
            &pool,
            &CreateReadingSessionInput {
                work_id: work_id.clone(),
                edition_id: None,
                status: "reading".into(),
                started_at: Some("2025-03-01".into()),
            },
        )
        .await
        .unwrap();
        let dnf = update_session(
            &pool,
            &attempt.id,
            &UpdateReadingSessionInput {
                status: Some("dnf".into()),
                started_at: None,
                finished_at: None,
                rating: None,
                review: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(dnf.status, "dnf");

        // Restart: brand-new session; the DNF session stays as-is.
        let restart = create_session(
            &pool,
            &CreateReadingSessionInput {
                work_id: work_id.clone(),
                edition_id: None,
                status: "reading".into(),
                started_at: None, // backfills started_at
            },
        )
        .await
        .unwrap();
        assert!(restart.started_at.is_some());
        let sessions = list_sessions_for_work(&pool, &work_id).await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().any(|s| s.status == "dnf"));
        assert!(sessions.iter().any(|s| s.status == "reading"));
    }

    #[tokio::test]
    async fn progress_events_are_recorded_and_ordered() {
        let pool = pool().await;
        let work_id = seed_work(&pool).await;
        let session = create_session(
            &pool,
            &CreateReadingSessionInput {
                work_id,
                edition_id: None,
                status: "reading".into(),
                started_at: None,
            },
        )
        .await
        .unwrap();

        add_progress(
            &pool,
            &session.id,
            &AddProgressInput {
                page: Some(42),
                percent: None,
                occurred_at: Some("2026-10-01T10:00:00Z".into()),
                note: None,
            },
        )
        .await
        .unwrap();
        let updated = add_progress(
            &pool,
            &session.id,
            &AddProgressInput {
                page: None,
                percent: Some(50),
                occurred_at: Some("2026-10-02T10:00:00Z".into()),
                note: Some("chapter 7".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.progress_events.len(), 2);
        assert_eq!(updated.progress_events[0].page, Some(42));
        assert_eq!(updated.progress_events[1].percent, Some(50));

        // Progress without page or percent is rejected.
        let err = add_progress(
            &pool,
            &session.id,
            &AddProgressInput {
                page: None,
                percent: None,
                occurred_at: None,
                note: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn invalid_status_and_unknown_work_rejected() {
        let pool = pool().await;
        let err = create_session(
            &pool,
            &CreateReadingSessionInput {
                work_id: "missing".into(),
                edition_id: None,
                status: "reading".into(),
                started_at: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));

        let work_id = seed_work(&pool).await;
        let err = create_session(
            &pool,
            &CreateReadingSessionInput {
                work_id,
                edition_id: None,
                status: "lent".into(), // ownership word, not a reading status
                started_at: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
