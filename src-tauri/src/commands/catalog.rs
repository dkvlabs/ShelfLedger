//! Thin IPC wrappers: no business logic here, all in `domain`.

use sqlx;
use tauri::State;

use crate::domain;
use crate::error::AppResult;
use crate::models::{
    AddBookInput, AddBookResult, CopyDto, CreateCopyInput, CreateEditionInput, CreateWorkInput,
    DuplicateQuery, EditionDto, UpdateCopyInput, UpdateWorkInput, WorkDetail, WorkSummary,
};
use crate::AppState;

/// Transactional workflow A: work + edition + optional copy in one command.
#[tauri::command]
pub async fn add_book(state: State<'_, AppState>, input: AddBookInput) -> AppResult<AddBookResult> {
    domain::catalog::create_book(&state.pool, &input).await
}

#[tauri::command]
pub async fn search_works(
    state: State<'_, AppState>,
    query: Option<String>,
) -> AppResult<Vec<WorkSummary>> {
    let pool = &state.pool;
    match query.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(q) => {
            let ids = crate::repositories::works::search_fts_ids(pool, q).await?;
            crate::repositories::works::get_work_summaries(pool, &ids).await
        }
        None => crate::repositories::works::list_work_summaries(pool).await,
    }
}

#[tauri::command]
pub async fn get_work(state: State<'_, AppState>, id: String) -> AppResult<WorkDetail> {
    domain::catalog::get_work_detail(&state.pool, &id).await
}

#[tauri::command]
pub async fn create_work(
    state: State<'_, AppState>,
    input: CreateWorkInput,
) -> AppResult<WorkDetail> {
    domain::catalog::create_work(&state.pool, &input).await
}

#[tauri::command]
pub async fn update_work(
    state: State<'_, AppState>,
    id: String,
    input: UpdateWorkInput,
) -> AppResult<WorkDetail> {
    domain::catalog::update_work(&state.pool, &id, &input).await
}

#[tauri::command]
pub async fn create_edition(
    state: State<'_, AppState>,
    input: CreateEditionInput,
) -> AppResult<EditionDto> {
    domain::catalog::create_edition(&state.pool, &input).await
}

#[tauri::command]
pub async fn create_copy(state: State<'_, AppState>, input: CreateCopyInput) -> AppResult<CopyDto> {
    domain::catalog::create_copy(&state.pool, &input).await
}

#[tauri::command]
pub async fn update_copy(
    state: State<'_, AppState>,
    id: String,
    input: UpdateCopyInput,
) -> AppResult<CopyDto> {
    domain::catalog::update_copy(&state.pool, &id, &input).await
}

#[tauri::command]
pub async fn archive_copy(state: State<'_, AppState>, id: String) -> AppResult<()> {
    domain::catalog::archive_copy(&state.pool, &id).await
}

#[tauri::command]
pub async fn find_duplicates(
    state: State<'_, AppState>,
    query: DuplicateQuery,
) -> AppResult<Vec<WorkSummary>> {
    domain::catalog::find_duplicates(&state.pool, &query).await
}

/// Convenience command for the dashboard: counts of works and active copies.
#[tauri::command]
pub async fn get_library_stats(
    state: State<'_, AppState>,
) -> AppResult<crate::models::LibraryStats> {
    let pool = &state.pool;
    let (work_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM works")
        .fetch_one(pool)
        .await?;
    let (copy_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM copies WHERE archived_at IS NULL")
            .fetch_one(pool)
            .await?;
    Ok(crate::models::LibraryStats {
        work_count,
        copy_count,
    })
}
