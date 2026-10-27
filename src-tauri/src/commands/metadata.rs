use tauri::State;

use crate::error::AppResult;
use crate::metadata::openlibrary;
use crate::models::LookupIsbnResult;
use crate::AppState;

/// User-initiated ISBN metadata lookup (Open Library, cached). Network errors
/// surface as `AppError::Network` so the UI can offer manual entry offline.
#[tauri::command]
pub async fn lookup_isbn(
    state: State<'_, AppState>,
    isbn: String,
) -> AppResult<Option<LookupIsbnResult>> {
    openlibrary::lookup_isbn(&state.pool, &isbn).await
}
