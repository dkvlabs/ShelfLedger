use tauri::State;

use crate::domain;
use crate::error::AppResult;
use crate::models::{
    AddProgressInput, CreateReadingSessionInput, ReadingSessionDto, UpdateReadingSessionInput,
};
use crate::AppState;

#[tauri::command]
pub async fn list_reading_sessions(
    state: State<'_, AppState>,
    status: Option<String>,
) -> AppResult<Vec<ReadingSessionDto>> {
    domain::reading::list_sessions(&state.pool, status.as_deref()).await
}

#[tauri::command]
pub async fn list_reading_sessions_for_work(
    state: State<'_, AppState>,
    work_id: String,
) -> AppResult<Vec<ReadingSessionDto>> {
    domain::reading::list_sessions_for_work(&state.pool, &work_id).await
}

#[tauri::command]
pub async fn create_reading_session(
    state: State<'_, AppState>,
    input: CreateReadingSessionInput,
) -> AppResult<ReadingSessionDto> {
    domain::reading::create_session(&state.pool, &input).await
}

#[tauri::command]
pub async fn update_reading_session(
    state: State<'_, AppState>,
    id: String,
    input: UpdateReadingSessionInput,
) -> AppResult<ReadingSessionDto> {
    domain::reading::update_session(&state.pool, &id, &input).await
}

#[tauri::command]
pub async fn add_reading_progress(
    state: State<'_, AppState>,
    session_id: String,
    input: AddProgressInput,
) -> AppResult<ReadingSessionDto> {
    domain::reading::add_progress(&state.pool, &session_id, &input).await
}
