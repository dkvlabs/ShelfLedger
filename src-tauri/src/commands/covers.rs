use crate::{domain, error::AppResult, models::CoverImage, repositories, AppState};
use tauri::State;

#[tauri::command]
pub async fn get_cover(state: State<'_, AppState>, path: String) -> AppResult<Option<CoverImage>> {
    repositories::covers::get(&state.pool, &path).await
}
#[tauri::command]
pub async fn save_cover_photo(
    state: State<'_, AppState>,
    edition_id: String,
    data_url: String,
) -> AppResult<String> {
    domain::covers::save_photo(&state.pool, &edition_id, data_url).await
}
#[tauri::command]
pub async fn fetch_cover(
    state: State<'_, AppState>,
    edition_id: String,
) -> AppResult<Option<String>> {
    domain::covers::fetch(&state.pool, &edition_id).await
}
#[tauri::command]
pub async fn remove_cover(state: State<'_, AppState>, edition_id: String) -> AppResult<()> {
    repositories::covers::remove(&state.pool, &edition_id).await
}
