//! HTTP transport for the web version: the same Rust core the desktop app
//! uses, served to a browser over loopback HTTP.
//!
//! `POST /api/<command>` accepts the same argument object as Tauri's
//! `invoke()` (camelCase keys) and answers with the same DTO JSON, or
//! `{kind, message}` with a matching status code on error. The typed wrappers
//! in `src/lib/api.ts` therefore work unchanged in both modes. This module is
//! thin transport only — all business logic stays in `domain/` and
//! `repositories/`, mirroring `commands/`.
//!
//! The server binds 127.0.0.1 only. Uploads land under the app data
//! directory with server-generated names; downloads are restricted to the
//! `exports/` and `backups/` directories the export commands write to.

use crate::sqlite_db as sqlx;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path as AxPath, Query as AxQuery, State as AxState};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::commands;
use crate::db;
use crate::domain;
use crate::error::{AppError, AppResult};
use crate::metadata;
use crate::models::{
    AddBookInput, AddProgressInput, AddValuationInput, CreateContactInput, CreateCopyInput,
    CreateEditionInput, CreateListingInput, CreateLocationInput, CreateReadingSessionInput,
    CreateWorkInput, DuplicateQuery, ImportCommitInput, LendCopyInput, RecordDisposalInput,
    ReturnCopyInput, UpdateContactInput, UpdateCopyInput, UpdateReadingSessionInput,
    UpdateWorkInput,
};
use crate::repositories;

/// Everything the HTTP layer needs: the library pool plus the local data
/// directory (uploads, exports, backups) and, optionally, the built frontend.
pub struct WebCtx {
    pub pool: sqlx::SqlitePool,
    pub data_dir: PathBuf,
    pub dist: Option<PathBuf>,
}

const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Argument extraction (same camelCase keys as the Tauri invoke payloads)
// ---------------------------------------------------------------------------

fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> AppResult<T> {
    let raw = args.get(key).cloned().unwrap_or(Value::Null);
    serde_json::from_value(raw)
        .map_err(|e| AppError::Validation(format!("bad argument {key:?}: {e}")))
}

fn arg_opt<T: DeserializeOwned>(args: &Value, key: &str) -> AppResult<Option<T>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(raw) => serde_json::from_value(raw.clone())
            .map(Some)
            .map_err(|e| AppError::Validation(format!("bad argument {key:?}: {e}"))),
    }
}

fn ok<T: serde::Serialize>(value: T) -> AppResult<Value> {
    Ok(serde_json::to_value(value)?)
}

// ---------------------------------------------------------------------------
// Command dispatch — one arm per named command, mirroring `commands/`
// ---------------------------------------------------------------------------

pub async fn dispatch(ctx: &WebCtx, cmd: &str, args: &Value) -> AppResult<Value> {
    let pool = &ctx.pool;
    match cmd {
        "list_market_comparisons" => {
            ok(domain::market::report(pool, &arg::<String>(args, "editionId")?).await?)
        }
        "save_market_comparison" => ok(domain::market::save(
            pool,
            &arg::<domain::market::SaveMarketComparableInput>(args, "input")?,
        )
        .await?),
        "set_market_comparison_included" => ok(domain::market::set_included(
            pool,
            &arg::<String>(args, "id")?,
            arg::<bool>(args, "included")?,
        )
        .await?),
        "save_market_estimate" => ok(domain::market::save_estimate(
            pool,
            &arg::<domain::market::SaveMarketEstimateInput>(args, "input")?,
        )
        .await?),
        "unarchive_copy" => {
            ok(domain::catalog::unarchive_copy(pool, &arg::<String>(args, "id")?).await?)
        }
        "update_edition" => ok(domain::catalog::update_edition(
            pool,
            &arg::<String>(args, "id")?,
            &arg::<CreateEditionInput>(args, "input")?,
        )
        .await?),
        "update_copy_details" => ok(domain::catalog::update_copy_details(
            pool,
            &arg::<String>(args, "id")?,
            &arg::<CreateCopyInput>(args, "input")?,
        )
        .await?),
        "list_work_tags" => {
            ok(domain::catalog::list_work_tags(pool, &arg::<String>(args, "id")?).await?)
        }
        "set_work_tags" => ok(domain::catalog::set_work_tags(
            pool,
            &arg::<String>(args, "id")?,
            &arg::<Vec<String>>(args, "tags")?,
        )
        .await?),
        "list_libraries" => ok(domain::profiles::list(pool, &ctx.data_dir).await?),
        "switch_library" => {
            ok(domain::profiles::switch(pool, &ctx.data_dir, &arg::<String>(args, "id")?).await?)
        }
        "preview_import" => ok(commands::data::preview_import_impl(
            pool,
            &arg::<String>(args, "path")?,
            &arg(args, "mapping")?,
        )
        .await?),
        "commit_import" => ok(commands::data::commit_import_impl(
            pool,
            &arg::<String>(args, "path")?,
            &arg(args, "mapping")?,
            &arg::<ImportCommitInput>(args, "input")?,
        )
        .await?),
        // catalog
        "search_works" => {
            ok(commands::catalog::search_works_impl(pool, arg_opt(args, "query")?).await?)
        }
        "get_work" => {
            ok(domain::catalog::get_work_detail(pool, &arg::<String>(args, "id")?).await?)
        }
        "add_book" => {
            ok(domain::catalog::create_book(pool, &arg::<AddBookInput>(args, "input")?).await?)
        }
        "create_work" => {
            ok(domain::catalog::create_work(pool, &arg::<CreateWorkInput>(args, "input")?).await?)
        }
        "update_work" => {
            let id = arg::<String>(args, "id")?;
            let input = arg::<UpdateWorkInput>(args, "input")?;
            ok(domain::catalog::update_work(pool, &id, &input).await?)
        }
        "create_edition" => ok(domain::catalog::create_edition(
            pool,
            &arg::<CreateEditionInput>(args, "input")?,
        )
        .await?),
        "create_copy" => {
            ok(domain::catalog::create_copy(pool, &arg::<CreateCopyInput>(args, "input")?).await?)
        }
        "update_copy" => {
            let id = arg::<String>(args, "id")?;
            let input = arg::<UpdateCopyInput>(args, "input")?;
            ok(domain::catalog::update_copy(pool, &id, &input).await?)
        }
        "archive_copy" => {
            domain::catalog::archive_copy(pool, &arg::<String>(args, "id")?).await?;
            ok(())
        }
        "find_duplicates" => ok(domain::catalog::find_duplicates(
            pool,
            &arg::<DuplicateQuery>(args, "query")?,
        )
        .await?),
        "get_library_stats" => ok(commands::catalog::get_library_stats_impl(pool).await?),

        // locations
        "list_locations" => ok(repositories::locations::list_locations(pool).await?),
        "create_location" => {
            let input = arg::<CreateLocationInput>(args, "input")?;
            input.validate().map_err(AppError::Validation)?;
            ok(repositories::locations::insert_location(pool, &input).await?)
        }
        "delete_location" => {
            ok(repositories::locations::delete_location(pool, &arg::<String>(args, "id")?).await?)
        }

        // settings
        "get_settings" => ok(repositories::settings::all(pool).await?),
        "set_setting" => {
            let key = arg::<String>(args, "key")?;
            let value = arg::<Value>(args, "value")?;
            repositories::settings::set_json(pool, &key, &value).await?;
            ok(())
        }

        // Metadata: user-initiated, cached, shared with native transports.
        "lookup_isbn" => ok(metadata::lookup_isbn(pool, &arg::<String>(args, "isbn")?).await?),
        "lookup_libris_book" => ok(metadata::libris::lookup(
            pool,
            &arg::<String>(args, "url")?,
            arg_opt::<String>(args, "isbn")?.as_deref(),
        )
        .await?),

        // reading
        "list_reading_sessions" => {
            let status = arg_opt::<String>(args, "status")?;
            ok(domain::reading::list_sessions(pool, status.as_deref()).await?)
        }
        "list_reading_sessions_for_work" => ok(domain::reading::list_sessions_for_work(
            pool,
            &arg::<String>(args, "workId")?,
        )
        .await?),
        "create_reading_session" => ok(domain::reading::create_session(
            pool,
            &arg::<CreateReadingSessionInput>(args, "input")?,
        )
        .await?),
        "update_reading_session" => {
            let id = arg::<String>(args, "id")?;
            let input = arg::<UpdateReadingSessionInput>(args, "input")?;
            ok(domain::reading::update_session(pool, &id, &input).await?)
        }
        "add_reading_progress" => {
            let session_id = arg::<String>(args, "sessionId")?;
            let input = arg::<AddProgressInput>(args, "input")?;
            ok(domain::reading::add_progress(pool, &session_id, &input).await?)
        }

        // loans
        "list_contacts" => ok(domain::loans::list_contacts(pool).await?),
        "create_contact" => ok(domain::loans::create_contact(
            pool,
            &arg::<CreateContactInput>(args, "input")?,
        )
        .await?),
        "update_contact" => {
            let id = arg::<String>(args, "id")?;
            let input = arg::<UpdateContactInput>(args, "input")?;
            ok(domain::loans::update_contact(pool, &id, &input).await?)
        }
        "list_loans" => {
            ok(domain::loans::list_loans(pool, &arg::<String>(args, "localToday")?).await?)
        }
        "list_active_loans" => {
            ok(domain::loans::list_active_loans(pool, &arg::<String>(args, "localToday")?).await?)
        }
        "list_overdue_loans" => ok(domain::loans::list_overdue_loans(
            pool,
            &arg::<String>(args, "localToday")?,
        )
        .await?),
        "lend_copy" => {
            ok(domain::loans::lend_copy(pool, &arg::<LendCopyInput>(args, "input")?).await?)
        }
        "return_loan" => {
            let loan_id = arg::<String>(args, "loanId")?;
            let input = arg::<ReturnCopyInput>(args, "input")?;
            ok(domain::loans::return_copy(pool, &loan_id, &input).await?)
        }

        // finance
        "add_valuation" => ok(domain::finance::add_valuation(
            pool,
            &arg::<AddValuationInput>(args, "input")?,
        )
        .await?),
        "list_valuations" => {
            ok(domain::finance::list_valuations(pool, &arg::<String>(args, "copyId")?).await?)
        }
        "create_listing" => ok(domain::finance::create_listing(
            pool,
            &arg::<CreateListingInput>(args, "input")?,
        )
        .await?),
        "withdraw_listing" => {
            domain::finance::withdraw_listing(pool, &arg::<String>(args, "listingId")?).await?;
            ok(())
        }
        "list_listings" => {
            ok(domain::finance::list_listings(pool, &arg::<String>(args, "copyId")?).await?)
        }
        "record_disposal" => ok(domain::finance::record_disposal(
            pool,
            &arg::<RecordDisposalInput>(args, "input")?,
        )
        .await?),
        "list_disposals" => {
            ok(domain::finance::list_disposals(pool, &arg::<String>(args, "copyId")?).await?)
        }
        "get_sell_queue" => ok(domain::finance::sell_queue(pool).await?),
        "set_sell_override" => {
            let copy_id = arg::<String>(args, "copyId")?;
            let override_value = arg_opt::<String>(args, "overrideValue")?;
            commands::finance::set_sell_override_impl(pool, &copy_id, override_value.as_deref())
                .await?;
            ok(())
        }
        "get_finance_report" => ok(domain::finance::finance_report(pool).await?),
        "get_dashboard" => {
            ok(domain::dashboard::get_dashboard(pool, &arg::<String>(args, "localToday")?).await?)
        }

        // data portability (files live under the local data directory)
        "export_library_csv" => {
            ok(commands::data::export_library_csv_impl(pool, &ctx.data_dir).await?)
        }
        "export_library_json" => {
            ok(commands::data::export_library_json_impl(pool, &ctx.data_dir).await?)
        }
        "import_library_json" => ok(commands::data::import_library_json_impl(
            pool,
            &arg::<String>(args, "path")?,
        )
        .await?),
        "preview_goodreads_import" => ok(commands::data::preview_goodreads_import_impl(
            pool,
            &arg::<String>(args, "path")?,
        )
        .await?),
        "commit_goodreads_import" => {
            let path = arg::<String>(args, "path")?;
            let input = arg::<ImportCommitInput>(args, "input")?;
            ok(commands::data::commit_goodreads_import_impl(pool, &path, &input).await?)
        }
        "create_backup" => {
            let output_path = arg_opt::<String>(args, "outputPath")?;
            ok(commands::data::create_backup_impl(pool, &ctx.data_dir, output_path).await?)
        }
        "restore_backup" => {
            let archive_path = arg::<String>(args, "archivePath")?;
            let mode = arg::<String>(args, "mode")?;
            ok(
                commands::data::restore_backup_impl(pool, &ctx.data_dir, &archive_path, &mode)
                    .await?,
            )
        }

        _ => Err(AppError::NotFound(format!("unknown command {cmd:?}"))),
    }
}

// ---------------------------------------------------------------------------
// HTTP surface
// ---------------------------------------------------------------------------

pub fn router(ctx: WebCtx) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route(
            "/api/session",
            get(|| async { Json(json!({"token": session_token()})) }),
        )
        .route("/api/upload", post(upload))
        .route("/api/download", get(download))
        .route("/api/{command}", post(invoke))
        .fallback(spa)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(axum::middleware::from_fn(guard_request))
        .with_state(Arc::new(ctx))
}

fn session_token() -> &'static str {
    static TOKEN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TOKEN.get_or_init(crate::ids::new_id)
}

fn trusted_headers(headers: &axum::http::HeaderMap) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Ok(authority) = host.parse::<axum::http::uri::Authority>() else {
        return false;
    };
    if !matches!(authority.host(), "localhost" | "127.0.0.1" | "[::1]") {
        return false;
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        let Ok(origin) = origin.to_str() else {
            return false;
        };
        let same_origin = origin == format!("http://{host}");
        let development_origin = cfg!(debug_assertions)
            && matches!(origin, "http://localhost:1420" | "http://127.0.0.1:1420");
        if !same_origin && !development_origin {
            return false;
        }
    }
    true
}

async fn guard_request(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    if !trusted_headers(request.headers()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if request.uri().path().starts_with("/api/") && request.method() == axum::http::Method::POST {
        if request
            .headers()
            .get("x-shelfledger-token")
            .and_then(|v| v.to_str().ok())
            != Some(session_token())
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        if request.uri().path() != "/api/upload"
            && !request
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.split(';').next() == Some("application/json"))
        {
            return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
        }
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    response
        .headers_mut()
        .insert(header::X_FRAME_OPTIONS, "DENY".parse().unwrap());
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

/// Start the web version: local database under `data_dir`, loopback HTTP.
pub async fn serve(data_dir: PathBuf, dist: Option<PathBuf>, port: u16) -> AppResult<()> {
    std::fs::create_dir_all(&data_dir)?;
    let pool = db::connect(&data_dir.join("shelfledger.db")).await?;
    let app = router(WebCtx {
        pool,
        data_dir,
        dist,
    });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    println!("ShelfLedger web version listening on http://127.0.0.1:{port} (loopback only)");
    axum::serve(listener, app)
        .await
        .map_err(|e| AppError::Other(format!("http server error: {e}")))
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "app": "shelfledger" }))
}

async fn invoke(
    AxState(ctx): AxState<Arc<WebCtx>>,
    AxPath(command): AxPath<String>,
    body: Bytes,
) -> Response {
    let args: Value = if body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                return err_response(AppError::Validation(format!(
                    "request body is not valid JSON: {e}"
                )))
            }
        }
    };
    match dispatch(&ctx, &command, &args).await {
        Ok(value) => ok_response(value),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct UploadQuery {
    name: String,
}

/// Browser file picker fallback: the file body is stored under a generated
/// name in the local data directory and the path is handed to the regular
/// commands, so import/restore logic is identical in both modes.
async fn upload(
    AxState(ctx): AxState<Arc<WebCtx>>,
    AxQuery(q): AxQuery<UploadQuery>,
    body: Bytes,
) -> Response {
    match handle_upload(&ctx, &q.name, &body).await {
        Ok(path) => Json(json!({ "path": path })).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Deserialize)]
struct DownloadQuery {
    path: String,
}

async fn download(
    AxState(ctx): AxState<Arc<WebCtx>>,
    AxQuery(q): AxQuery<DownloadQuery>,
) -> Response {
    match resolve_download(&ctx, &q.path) {
        Some(path) => match std::fs::read(&path) {
            Ok(bytes) => file_response(&path, bytes),
            Err(e) => err_response(AppError::Io(e)),
        },
        None => err_response(AppError::NotFound(
            "file is not available for download".into(),
        )),
    }
}

async fn spa(AxState(ctx): AxState<Arc<WebCtx>>, uri: Uri) -> Response {
    let path = uri.path();
    if let Some(rest) = path.strip_prefix("/api/") {
        return err_response(AppError::NotFound(format!("unknown endpoint /api/{rest}")));
    }
    match serve_dist(ctx.dist.as_deref(), path) {
        Some((content_type, bytes)) => {
            ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
        }
        None => err_response(AppError::NotFound(
            "frontend not built — run `pnpm build` (or set SHELFLEDGER_WEB_DIST)".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Upload / download / static-file helpers
// ---------------------------------------------------------------------------

/// Keep only the basename and a conservative character set, then strip
/// leading/trailing dots. The stored path is generated server-side, so this
/// can never escape the upload directory.
pub(crate) fn sanitize_upload_name(name: &str) -> AppResult<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    let filtered: String = base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    let safe = filtered.trim_matches('.');
    if safe.is_empty() || safe.len() > 128 {
        return Err(AppError::Validation(format!(
            "unsupported file name {name:?}"
        )));
    }
    Ok(safe.to_string())
}

pub(crate) async fn handle_upload(ctx: &WebCtx, name: &str, body: &[u8]) -> AppResult<String> {
    let safe = sanitize_upload_name(name)?;
    let dir = crate::paths::write_path(&ctx.data_dir, &ctx.data_dir.join("web-uploads"))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}-{}", crate::ids::new_id(), safe));
    std::fs::write(&path, body)?;
    Ok(path.to_string_lossy().to_string())
}

/// Downloads are deliberately restricted to the `exports/` and `backups/`
/// directories our own export commands write to — never arbitrary paths.
pub(crate) fn resolve_download(ctx: &WebCtx, requested: &str) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(requested).ok()?;
    if !canonical.is_file() {
        return None;
    }
    for sub in ["exports", "backups"] {
        if let Ok(root) = std::fs::canonicalize(ctx.data_dir.join(sub)) {
            if canonical.starts_with(&root) {
                return Some(canonical);
            }
        }
    }
    None
}

fn file_response(path: &Path, bytes: Vec<u8>) -> Response {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".into());
    let content_type = match path.extension().and_then(|e| e.to_str()) {
        Some("csv") => "text/csv; charset=utf-8",
        Some("json") => "application/json",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    };
    (
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_DISPOSITION,
                // file_name() already strips directories; quotes guard odd chars
                &format!("attachment; filename=\"{name}\""),
            ),
        ],
        bytes,
    )
        .into_response()
}

/// Serve the built frontend (`dist/`); unknown-but-valid paths fall back to
/// `index.html` so client-side routes work on reload. Traversal-shaped paths
/// are rejected outright (no fallback).
pub(crate) fn serve_dist(dist: Option<&Path>, url_path: &str) -> Option<(&'static str, Vec<u8>)> {
    let dist = dist?;
    let root = std::fs::canonicalize(dist).ok()?;
    let rel = url_path.trim_start_matches('/');
    let mut target = dist.to_path_buf();
    if rel.is_empty() {
        target.push("index.html");
    } else {
        for segment in rel.split('/') {
            if segment.is_empty()
                || segment == ".."
                || segment.starts_with('.')
                || !segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@'))
            {
                return None;
            }
            target.push(segment);
        }
    }
    read_file_under(&root, &target).or_else(|| read_file_under(&root, &dist.join("index.html")))
}

fn read_file_under(root: &Path, candidate: &Path) -> Option<(&'static str, Vec<u8>)> {
    let canonical = std::fs::canonicalize(candidate).ok()?;
    if !canonical.is_file() || !canonical.starts_with(root) {
        return None;
    }
    let bytes = std::fs::read(&canonical).ok()?;
    Some((content_type_for(&canonical), bytes))
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") | Some("map") => "application/json",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

// ---------------------------------------------------------------------------
// Response mapping (matches the Tauri `{kind, message}` error contract)
// ---------------------------------------------------------------------------

fn ok_response(value: Value) -> Response {
    if value.is_null() {
        StatusCode::NO_CONTENT.into_response()
    } else {
        Json(value).into_response()
    }
}

fn err_response(err: AppError) -> Response {
    (status_for(&err), Json(&err)).into_response()
}

fn status_for(err: &AppError) -> StatusCode {
    match err {
        AppError::Validation(_) => StatusCode::BAD_REQUEST,
        AppError::NotFound(_) => StatusCode::NOT_FOUND,
        AppError::Conflict(_) => StatusCode::CONFLICT,
        AppError::Network(_) => StatusCode::BAD_GATEWAY,
        AppError::MetadataDisabled => StatusCode::FORBIDDEN,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn make_ctx() -> WebCtx {
        let pool = db::connect_memory().await.expect("in-memory db");
        let data_dir =
            std::env::temp_dir().join(format!("shelfledger-web-test-{}", crate::ids::new_id()));
        std::fs::create_dir_all(&data_dir).expect("temp dir");
        WebCtx {
            pool,
            data_dir,
            dist: None,
        }
    }

    fn cleanup(ctx: WebCtx) {
        let _ = std::fs::remove_dir_all(&ctx.data_dir);
    }

    #[tokio::test]
    async fn dispatch_libris_lookup_uses_cache_and_validates_its_arguments() {
        let ctx = make_ctx().await;
        repositories::settings::set_json(&ctx.pool, "metadata.lookup_enabled", &json!(false))
            .await
            .unwrap();
        let url = "https://www.libris.ro/carte/mitologie-nordica-neil-gaiman/1258850";
        let result = json!({"source":"libris","isbn13":"9786067882629","isbn10":"6067882620",
            "title":"Mitologie nordica","subtitle":null,"publisher":"GRUPUL EDITORIAL ART",
            "publishedAt":"2017","language":"ro","pageCount":250,"authorNames":["Neil Gaiman"],"cached":false});
        sqlx::query(
            "INSERT INTO metadata_cache (cache_key,payload_json,fetched_at) VALUES (?1,?2,?3)",
        )
        .bind(format!("libris:url:{url}"))
        .bind(result.to_string())
        .bind(crate::ids::now_iso())
        .execute(&ctx.pool)
        .await
        .unwrap();
        let found = dispatch(
            &ctx,
            "lookup_libris_book",
            &json!({"url":url,"isbn":"978-606-788-262-9"}),
        )
        .await
        .unwrap();
        assert_eq!(found["title"], "Mitologie nordica");
        assert_eq!(found["cached"], true);
        assert!(matches!(
            dispatch(
                &ctx,
                "lookup_libris_book",
                &json!({"url":url,"isbn":"9780306406157"})
            )
            .await,
            Err(AppError::Validation(_))
        ));
        assert!(dispatch(
            &ctx,
            "lookup_libris_book",
            &json!({"url":"https://example.com/book","isbn":null})
        )
        .await
        .is_err());
        cleanup(ctx);
    }

    #[tokio::test]
    async fn dispatch_add_book_round_trip() {
        let ctx = make_ctx().await;
        let created = dispatch(
            &ctx,
            "add_book",
            &json!({
                "input": {
                    "title": "The Left Hand of Darkness",
                    "authorNames": ["Ursula K. Le Guin"],
                    "isbn13": "9780441478125",
                    "copy": { "conditionGrade": "good" }
                }
            }),
        )
        .await
        .expect("add_book");
        assert!(created["workId"].is_string());
        assert_eq!(created["warning"], Value::Null);

        let found = dispatch(&ctx, "search_works", &json!({ "query": "Left Hand" }))
            .await
            .expect("search");
        let rows = found.as_array().expect("array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["title"], "The Left Hand of Darkness");
        assert_eq!(rows[0]["authorNames"][0], "Ursula K. Le Guin");

        let detail = dispatch(&ctx, "get_work", &json!({ "id": created["workId"] }))
            .await
            .expect("get_work");
        assert_eq!(detail["work"]["title"], "The Left Hand of Darkness");
        assert_eq!(detail["editions"][0]["isbn13"], "9780441478125");
        assert_eq!(detail["editions"][0]["copies"].as_array().unwrap().len(), 1);

        let stats = dispatch(&ctx, "get_library_stats", &json!({}))
            .await
            .expect("stats");
        assert_eq!(stats["workCount"], 1);
        assert_eq!(stats["copyCount"], 1);
        cleanup(ctx);
    }

    #[tokio::test]
    async fn dispatch_rejects_unknown_command_and_bad_args() {
        let ctx = make_ctx().await;
        let err = dispatch(&ctx, "drop_tables", &json!({})).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)), "got {err:?}");

        let err = dispatch(&ctx, "get_work", &json!({ "id": 42 }))
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");

        let err = dispatch(&ctx, "add_book", &json!({ "input": { "title": "   " } }))
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
        cleanup(ctx);
    }

    #[test]
    fn error_status_mapping_matches_frontend_contract() {
        assert_eq!(
            status_for(&AppError::Validation("x".into())),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status_for(&AppError::NotFound("x".into())),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status_for(&AppError::Conflict("x".into())),
            StatusCode::CONFLICT
        );
        assert_eq!(
            status_for(&AppError::Network("x".into())),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            status_for(&AppError::MetadataDisabled),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status_for(&AppError::Other("x".into())),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn upload_is_stored_under_generated_name() {
        let ctx = make_ctx().await;
        let path = handle_upload(&ctx, "../../../etc/passwd", b"Title,Author")
            .await
            .expect("upload");
        let p = PathBuf::from(&path);
        assert!(p.starts_with(ctx.data_dir.canonicalize().unwrap().join("web-uploads")));
        assert!(p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("-passwd"));
        assert!(sanitize_upload_name("..").is_err());
        assert!(sanitize_upload_name("").is_err());
        assert_eq!(
            sanitize_upload_name("goodreads.csv").unwrap(),
            "goodreads.csv"
        );
        cleanup(ctx);
    }

    #[tokio::test]
    async fn download_is_limited_to_exports_and_backups() {
        let ctx = make_ctx().await;
        let exports = ctx.data_dir.join("exports");
        std::fs::create_dir_all(&exports).unwrap();
        let file = exports.join("out.csv");
        std::fs::write(&file, "a,b").unwrap();
        assert!(resolve_download(&ctx, &file.to_string_lossy()).is_some());

        let db = ctx.data_dir.join("shelfledger.db");
        std::fs::write(&db, "secret").unwrap();
        assert!(resolve_download(&ctx, &db.to_string_lossy()).is_none());
        assert!(resolve_download(&ctx, "/etc/passwd").is_none());
        cleanup(ctx);
    }

    #[test]
    fn static_paths_reject_traversal() {
        let dir =
            std::env::temp_dir().join(format!("shelfledger-dist-test-{}", crate::ids::new_id()));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), "<html></html>").unwrap();
        std::fs::write(dir.join("assets").join("app.js"), "// js").unwrap();

        assert!(serve_dist(Some(&dir), "/").is_some());
        assert!(serve_dist(Some(&dir), "/assets/app.js").is_some());
        assert!(serve_dist(Some(&dir), "/missing-route").is_some()); // SPA fallback
        assert!(serve_dist(Some(&dir), "/../Cargo.toml").is_none());
        assert!(serve_dist(Some(&dir), "/assets/../../Cargo.toml").is_none());
        assert!(serve_dist(None, "/").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod security_regressions {
    use super::*;
    #[tokio::test]
    async fn http_rejects_untrusted_origin_host_missing_token_and_simple_post() {
        let root = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir_all(&root).unwrap();
        let pool = crate::db::connect_memory().await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                router(WebCtx {
                    pool,
                    data_dir: root.clone(),
                    dist: None,
                }),
            )
            .await
            .unwrap()
        });
        let client = reqwest::Client::new();
        let url = format!("http://{addr}/api/get_library_stats");
        for (origin, host, token, content, status) in [
            (
                "https://untrusted.example",
                addr.to_string(),
                true,
                "application/json",
                403,
            ),
            ("", "attacker.example".into(), true, "application/json", 403),
            ("", addr.to_string(), false, "application/json", 403),
            ("", addr.to_string(), true, "text/plain", 415),
            ("", addr.to_string(), true, "application/json", 200),
        ] {
            let mut request = client
                .post(&url)
                .header("host", host)
                .header("content-type", content)
                .body("{}");
            if !origin.is_empty() {
                request = request.header("origin", origin);
            }
            if token {
                request = request.header("x-shelfledger-token", session_token());
            }
            let response = request.send().await.unwrap();
            assert_eq!(response.status().as_u16(), status);
        }
        let bootstrap = client
            .get(format!("http://{addr}/api/session"))
            .send()
            .await
            .unwrap();
        assert_eq!(bootstrap.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            bootstrap.json::<Value>().await.unwrap()["token"],
            session_token()
        );
        server.abort();
    }
}
