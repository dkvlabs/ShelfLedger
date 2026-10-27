use tauri::State;

use crate::error::AppResult;
use crate::models::{CreateLocationInput, LocationDto};
use crate::repositories::locations;
use crate::AppState;

#[tauri::command]
pub async fn list_locations(state: State<'_, AppState>) -> AppResult<Vec<LocationDto>> {
    locations::list_locations(&state.pool).await
}

#[tauri::command]
pub async fn create_location(
    state: State<'_, AppState>,
    input: CreateLocationInput,
) -> AppResult<LocationDto> {
    input
        .validate()
        .map_err(crate::error::AppError::Validation)?;
    locations::insert_location(&state.pool, &input).await
}

#[tauri::command]
pub async fn delete_location(state: State<'_, AppState>, id: String) -> AppResult<bool> {
    locations::delete_location(&state.pool, &id).await
}
