use serde_json::Value;
use tauri::State;

use crate::error::AppResult;
use crate::repositories::settings;
use crate::AppState;

#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
) -> AppResult<std::collections::HashMap<String, Value>> {
    settings::all(&state.pool).await
}

#[tauri::command]
pub async fn set_setting(state: State<'_, AppState>, key: String, value: Value) -> AppResult<()> {
    settings::set_json(&state.pool, &key, &value).await
}
