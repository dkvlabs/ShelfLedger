//! Export / backup / import commands. All file I/O happens in Rust; the
//! webview only ever passes paths chosen through the dialog plugin (in the
//! web version, paths come from `/api/upload`). The `*_impl` functions are
//! shared by the Tauri commands and the web dispatcher (`web::dispatch`).

use crate::sqlite_db as sqlx;
use std::path::Path;

use sqlx::SqlitePool;
use tauri::{Manager, State};

use crate::domain;
use crate::error::{AppError, AppResult};
use crate::models::{
    BackupResult, ExportResult, ImportCommitInput, ImportPreview, ImportResult, RestoreResult,
};
use crate::AppState;

fn data_dir(app: &tauri::AppHandle) -> AppResult<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|_| AppError::Other("app data directory is not resolvable".into()))
}

// ---------------------------------------------------------------------------
// Shared implementations
// ---------------------------------------------------------------------------

pub async fn export_library_csv_impl(
    pool: &SqlitePool,
    data_dir: &Path,
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
    .fetch_all(pool)
    .await?;
    let csv = domain::backup::build_copies_csv(rows);

    let dir = crate::paths::write_path(data_dir, &data_dir.join("exports"))?;
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!(
        "shelfledger-copies-{stamp}-{}.csv",
        crate::ids::new_id()
    ));
    std::fs::write(&path, csv)?;
    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        format: "csv".into(),
    })
}

/// Structured JSON export of the full library (works → editions → copies,
/// plus reading sessions), suitable for re-import via `import_library_json`.
pub async fn export_library_json_impl(
    pool: &SqlitePool,
    data_dir: &Path,
) -> AppResult<ExportResult> {
    let out = crate::repositories::snapshot::export(pool).await?;
    let dir = crate::paths::write_path(data_dir, &data_dir.join("exports"))?;
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!(
        "shelfledger-library-{stamp}-{}.json",
        crate::ids::new_id()
    ));
    let json = serde_json::to_string_pretty(&out)?;
    std::fs::write(&path, json)?;
    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        format: "json".into(),
    })
}

pub async fn import_library_json_impl(pool: &SqlitePool, path: &str) -> AppResult<ImportResult> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Validation(format!("could not read import file: {e}")))?;
    let result = domain::import::import_json(pool, &text).await?;
    if result.imported_works > 0 {
        // Let list views refresh.
        let _ = sqlx::query("SELECT 1").execute(pool).await;
    }
    Ok(result)
}

pub async fn preview_goodreads_import_impl(
    pool: &SqlitePool,
    path: &str,
) -> AppResult<ImportPreview> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Validation(format!("could not read import file: {e}")))?;
    let preview = domain::import::preview_goodreads_csv(&text)?;
    domain::import::annotate_duplicates(pool, preview).await
}

pub async fn commit_goodreads_import_impl(
    pool: &SqlitePool,
    path: &str,
    input: &ImportCommitInput,
) -> AppResult<ImportResult> {
    domain::import::commit_goodreads_csv(pool, path, input).await
}

/// Complete backup: `VACUUM INTO` snapshot + hashed manifest, zipped under
/// the app data directory (or a caller-chosen path).
pub async fn create_backup_impl(
    pool: &SqlitePool,
    data_dir: &Path,
    output_path: Option<String>,
) -> AppResult<BackupResult> {
    let db_path = data_dir.join("shelfledger.db");
    let target = match output_path {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let dir = data_dir.join("backups");
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            dir.join(format!("shelfledger-backup-{stamp}.zip"))
        }
    };
    let target = crate::paths::write_path(data_dir, &target)?;
    let path = domain::backup::create_backup(pool, &db_path, &target).await?;
    Ok(BackupResult {
        path: path.to_string_lossy().to_string(),
    })
}

pub async fn restore_backup_impl(
    pool: &SqlitePool,
    data_dir: &Path,
    archive_path: &str,
    mode: &str,
) -> AppResult<RestoreResult> {
    let db_path = data_dir.join("shelfledger.db");
    domain::backup::restore_archive(pool, &db_path, Path::new(archive_path), mode).await
}

// ---------------------------------------------------------------------------
// Thin IPC wrappers
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn export_library_csv(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ExportResult> {
    export_library_csv_impl(&state.pool, &data_dir(&app)?).await
}

#[tauri::command]
pub async fn export_library_json(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<ExportResult> {
    export_library_json_impl(&state.pool, &data_dir(&app)?).await
}

#[tauri::command]
pub async fn import_library_json(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<ImportResult> {
    import_library_json_impl(&state.pool, &path).await
}

#[tauri::command]
pub async fn preview_goodreads_import(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<ImportPreview> {
    preview_goodreads_import_impl(&state.pool, &path).await
}

#[tauri::command]
pub async fn commit_goodreads_import(
    state: State<'_, AppState>,
    path: String,
    input: ImportCommitInput,
) -> AppResult<ImportResult> {
    commit_goodreads_import_impl(&state.pool, &path, &input).await
}

#[tauri::command]
pub async fn create_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    output_path: Option<String>,
) -> AppResult<BackupResult> {
    create_backup_impl(&state.pool, &data_dir(&app)?, output_path).await
}

#[tauri::command]
pub async fn restore_backup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    archive_path: String,
    mode: String,
) -> AppResult<RestoreResult> {
    // Restore transactionally through the live pool; WAL remains valid.
    restore_backup_impl(&state.pool, &data_dir(&app)?, &archive_path, &mode).await
}

#[tauri::command]
pub async fn list_libraries(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Vec<domain::profiles::Profile>> {
    domain::profiles::list(&state.pool, &data_dir(&app)?).await
}
#[tauri::command]
pub async fn switch_library(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    domain::profiles::switch(&state.pool, &data_dir(&app)?, &id).await
}

fn mapped_csv(
    text: &str,
    mapping: &std::collections::BTreeMap<String, String>,
) -> AppResult<(String, Vec<String>)> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| AppError::Validation(e.to_string()))?
        .clone();
    let original = headers.iter().map(str::to_string).collect::<Vec<_>>();
    const FIELDS: &[&str] = &[
        "Title",
        "Author",
        "ISBN13",
        "ISBN",
        "Exclusive Shelf",
        "Bookshelves",
        "My Rating",
        "My Review",
        "Date Read",
    ];
    let mut sources = std::collections::BTreeSet::new();
    for (target, source) in mapping {
        if !FIELDS.contains(&target.as_str()) || (!source.is_empty() && !original.contains(source))
        {
            return Err(AppError::Validation("unknown import column mapping".into()));
        }
        if !source.is_empty() && !sources.insert(source) {
            return Err(AppError::Validation(
                "map each source column only once".into(),
            ));
        }
    }
    let renamed = headers
        .iter()
        .enumerate()
        .map(|(index, name)| {
            if let Some((target, _)) = mapping
                .iter()
                .find(|(_, source)| !source.is_empty() && source.as_str() == name)
            {
                target.to_string()
            } else if mapping.contains_key(name) {
                format!("__unmapped_{index}")
            } else {
                name.to_string()
            }
        })
        .collect::<Vec<_>>();
    let distinct = renamed.iter().collect::<std::collections::BTreeSet<_>>();
    if distinct.len() != renamed.len() {
        return Err(AppError::Validation(
            "duplicate import column names; map or unmap conflicting columns".into(),
        ));
    }
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record(renamed)
        .map_err(|e| AppError::Validation(e.to_string()))?;
    for record in reader.records() {
        writer
            .write_record(&record.map_err(|e| AppError::Validation(e.to_string()))?)
            .map_err(|e| AppError::Validation(e.to_string()))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    Ok((
        String::from_utf8(bytes).map_err(|e| AppError::Validation(e.to_string()))?,
        original,
    ))
}

pub async fn preview_import_impl(
    pool: &SqlitePool,
    path: &str,
    mapping: &std::collections::BTreeMap<String, String>,
) -> AppResult<serde_json::Value> {
    let text = std::fs::read_to_string(path)?;
    if path.to_lowercase().ends_with(".json") {
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| AppError::Validation(e.to_string()))?;
        let snapshot = value.is_object();
        let works = if snapshot {
            value
                .get("tables")
                .and_then(|t| t.get("works"))
                .and_then(|v| v.as_array())
        } else {
            value.as_array()
        }
        .ok_or_else(|| AppError::Validation("invalid JSON library".into()))?;
        let rows=works.iter().enumerate().map(|(i,w)|serde_json::json!({"rowNumber":i+1,"title":w.get("title").and_then(|v|v.as_str()).unwrap_or(""),"author":w.get("authorNames").and_then(|v|v.as_array()).map(|a|a.iter().filter_map(|v|v.as_str()).collect::<Vec<_>>().join("; ")).unwrap_or_default(),"isbn13":null,"shelf":null,"rating":null,"dateRead":null,"review":null,"tags":[],"warnings":[],"duplicateOf":null})).collect::<Vec<_>>();
        return Ok(
            serde_json::json!({"kind":if snapshot {"snapshot"} else {"json"},"headers":[],"rows":rows}),
        );
    }
    let (text, headers) = mapped_csv(&text, mapping)?;
    let preview =
        domain::import::annotate_duplicates(pool, domain::import::preview_goodreads_csv(&text)?)
            .await?;
    Ok(serde_json::json!({"kind":"csv","headers":headers,"rows":preview.rows}))
}
pub async fn commit_import_impl(
    pool: &SqlitePool,
    path: &str,
    mapping: &std::collections::BTreeMap<String, String>,
    input: &ImportCommitInput,
) -> AppResult<ImportResult> {
    let text = std::fs::read_to_string(path)?;
    if path.to_lowercase().ends_with(".json") {
        let mut value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| AppError::Validation(e.to_string()))?;
        if let Some(rows) = value.as_array_mut() {
            let mut number = 0;
            rows.retain(|_| {
                number += 1;
                input.row_numbers.contains(&number)
            });
        } else {
            let count = value
                .get("tables")
                .and_then(|t| t.get("works"))
                .and_then(|v| v.as_array())
                .map_or(0, Vec::len);
            if input.row_numbers.len() != count
                || (1..=count).any(|n| !input.row_numbers.contains(&n))
            {
                return Err(AppError::Validation(
                    "a complete snapshot must be imported together".into(),
                ));
            }
        }
        return domain::import::import_json(pool, &serde_json::to_string(&value)?).await;
    }
    let (text, _) = mapped_csv(&text, mapping)?;
    domain::import::commit_csv_text(pool, &text, input).await
}
#[tauri::command]
pub async fn preview_import(
    state: State<'_, AppState>,
    path: String,
    mapping: std::collections::BTreeMap<String, String>,
) -> AppResult<serde_json::Value> {
    preview_import_impl(&state.pool, &path, &mapping).await
}
#[tauri::command]
pub async fn commit_import(
    state: State<'_, AppState>,
    path: String,
    mapping: std::collections::BTreeMap<String, String>,
    input: ImportCommitInput,
) -> AppResult<ImportResult> {
    commit_import_impl(&state.pool, &path, &mapping, &input).await
}
