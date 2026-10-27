//! Export / backup / import commands. All file I/O happens in Rust; the
//! webview only ever passes paths chosen through the dialog plugin.

use tauri::{Manager, State};

use crate::domain;
use crate::error::{AppError, AppResult};
use crate::models::{
    BackupResult, ExportResult, ImportCommitInput, ImportPreview, ImportResult, RestoreResult,
};
use crate::AppState;

// sqlx row shapes for the JSON export.
type ExportEditionRow = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
);
type ExportCopyRow = (
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
);

fn data_dir(app: &tauri::AppHandle) -> AppResult<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|_| AppError::Other("app data directory is not resolvable".into()))
}

#[tauri::command]
pub async fn export_library_csv(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ExportResult> {
    let rows: Vec<domain::backup::CopyExportRow> = sqlx::query_as(
        "SELECT w.title, \
                coalesce((SELECT group_concat(a.name, ', ') FROM work_authors wa \
                          JOIN authors a ON a.id = wa.author_id WHERE wa.work_id = w.id), ''), \
                coalesce(e.isbn13, ''), coalesce(e.isbn10, ''), coalesce(e.publisher, ''), \
                c.condition_grade, c.ownership_status, coalesce(l.name, ''), \
                coalesce(c.acquired_at, ''), coalesce(ac.store, ''), \
                ac.price_minor, ac.shipping_minor, ac.tax_minor, ac.currency \
         FROM copies c \
         JOIN editions e ON e.id = c.edition_id \
         JOIN works w ON w.id = e.work_id \
         LEFT JOIN locations l ON l.id = c.location_id \
         LEFT JOIN acquisitions ac ON ac.copy_id = c.id \
         ORDER BY w.title",
    )
    .fetch_all(&state.pool)
    .await?;
    let csv = domain::backup::build_copies_csv(rows);

    let dir = data_dir(&app)?.join("exports");
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("shelfledger-copies-{stamp}.csv"));
    std::fs::write(&path, csv)?;
    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        format: "csv".into(),
    })
}

/// Structured JSON export of the full library (works → editions → copies,
/// plus reading sessions), suitable for re-import via `import_library_json`.
#[tauri::command]
pub async fn export_library_json(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ExportResult> {
    let works: Vec<(String, String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, title, subtitle, original_language FROM works ORDER BY title")
            .fetch_all(&state.pool)
            .await?;

    let mut out: Vec<serde_json::Value> = Vec::with_capacity(works.len());
    for (work_id, title, subtitle, original_language) in works {
        let author_names = crate::repositories::works::list_author_names(&state.pool, &work_id)
            .await?
            .join(", ");
        let editions: Vec<ExportEditionRow> = sqlx::query_as(
            "SELECT id, isbn10, isbn13, publisher, published_at, language, page_count, binding \
                 FROM editions WHERE work_id = ?1 ORDER BY created_at",
        )
        .bind(&work_id)
        .fetch_all(&state.pool)
        .await?;
        let mut editions_json = Vec::with_capacity(editions.len());
        for (edition_id, isbn10, isbn13, publisher, published_at, language, page_count, binding) in
            editions
        {
            let copies: Vec<ExportCopyRow> = sqlx::query_as(
                "SELECT c.condition_grade, coalesce(c.acquired_at, ''), c.note, \
                            ac.price_minor, ac.currency, ac.store \
                     FROM copies c LEFT JOIN acquisitions ac ON ac.copy_id = c.id \
                     WHERE c.edition_id = ?1 AND c.archived_at IS NULL",
            )
            .bind(&edition_id)
            .fetch_all(&state.pool)
            .await?;
            editions_json.push(serde_json::json!({
                "isbn10": isbn10,
                "isbn13": isbn13,
                "publisher": publisher,
                "publishedAt": published_at,
                "language": language,
                "pageCount": page_count,
                "binding": binding,
                "copies": copies.into_iter().map(|(condition_grade, acquired_at, note, price_minor, currency, store)| serde_json::json!({
                    "conditionGrade": condition_grade,
                    "acquiredAt": if acquired_at.is_empty() { None::<String> } else { Some(acquired_at) },
                    "note": note,
                    "priceMinor": price_minor,
                    "currency": currency,
                    "store": store,
                })).collect::<Vec<_>>(),
            }));
        }
        out.push(serde_json::json!({
            "title": title,
            "subtitle": subtitle,
            "originalLanguage": original_language,
            "authorNames": author_names
                .split(", ")
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>(),
            "editions": editions_json,
        }));
    }

    let dir = data_dir(&app)?.join("exports");
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("shelfledger-library-{stamp}.json"));
    let json = serde_json::to_string_pretty(&out)?;
    std::fs::write(&path, json)?;
    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        format: "json".into(),
    })
}

#[tauri::command]
pub async fn import_library_json(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<ImportResult> {
    let text = std::fs::read_to_string(&path)
        .map_err(|e| AppError::Validation(format!("could not read import file: {e}")))?;
    let result = domain::import::import_json(&state.pool, &text).await?;
    if result.imported_works > 0 {
        // Let list views refresh.
        let _ = sqlx::query("SELECT 1").execute(&state.pool).await;
    }
    Ok(result)
}

#[tauri::command]
pub async fn preview_goodreads_import(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<ImportPreview> {
    let text = std::fs::read_to_string(&path)
        .map_err(|e| AppError::Validation(format!("could not read import file: {e}")))?;
    let preview = domain::import::preview_goodreads_csv(&text)?;
    domain::import::annotate_duplicates(&state.pool, preview).await
}

#[tauri::command]
pub async fn commit_goodreads_import(
    state: State<'_, AppState>,
    path: String,
    input: ImportCommitInput,
) -> AppResult<ImportResult> {
    domain::import::commit_goodreads_csv(&state.pool, &path, &input).await
}

/// Complete backup: `VACUUM INTO` snapshot + hashed manifest, zipped under
/// the app data directory (or a caller-chosen path).
#[tauri::command]
pub async fn create_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    output_path: Option<String>,
) -> AppResult<BackupResult> {
    let db_path = data_dir(&app)?.join("shelfledger.db");
    let target = match output_path {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let dir = data_dir(&app)?.join("backups");
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            dir.join(format!("shelfledger-backup-{stamp}.zip"))
        }
    };
    let path = domain::backup::create_backup(&state.pool, &db_path, &target).await?;
    Ok(BackupResult {
        path: path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn restore_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    archive_path: String,
    mode: String,
) -> AppResult<RestoreResult> {
    // Hold the pool across restore: the swap happens on disk and the app
    // restarts to reopen the restored database.
    let _ = &state;
    let db_path = data_dir(&app)?.join("shelfledger.db");
    domain::backup::restore_archive(&db_path, std::path::Path::new(&archive_path), &mode).await
}
