use tauri::State;

use crate::error::AppResult;
use crate::metadata;
use crate::models::LookupIsbnResult;
use crate::AppState;

/// User-initiated ISBN metadata lookup with cached provider fallbacks. Network errors
/// surface as `AppError::Network` so the UI can offer manual entry offline.
#[tauri::command]
pub async fn lookup_isbn(
    state: State<'_, AppState>,
    isbn: String,
) -> AppResult<Option<LookupIsbnResult>> {
    metadata::lookup_isbn(&state.pool, &isbn).await
}

#[tauri::command]
pub async fn lookup_libris_book(
    state: State<'_, AppState>,
    url: String,
    isbn: Option<String>,
) -> AppResult<Option<LookupIsbnResult>> {
    metadata::libris::lookup(&state.pool, &url, isbn.as_deref()).await
}
