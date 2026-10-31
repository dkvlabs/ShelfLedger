use crate::{
    domain::updates,
    error::{AppError, AppResult},
};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn get_app_info() -> updates::AppInfo {
    updates::app_info()
}

#[tauri::command]
pub async fn check_for_updates() -> AppResult<updates::UpdateCheck> {
    updates::check_for_updates().await
}

#[tauri::command]
pub fn open_update_page(app: tauri::AppHandle, page: updates::UpdatePage) -> AppResult<()> {
    app.opener()
        .open_url(updates::page_url(page), None::<&str>)
        .map_err(|e| AppError::Other(e.to_string()))
}
