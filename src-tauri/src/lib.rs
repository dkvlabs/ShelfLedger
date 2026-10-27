use crate::sqlite_db as sqlx;
mod commands;
mod db;
mod domain;
mod error;
mod ids;
mod isbn;
mod metadata;
mod models;
mod money;
mod paths;
mod repositories;
mod sqlite_db;
pub mod web;

use tauri::Manager;

pub struct AppState {
    pub pool: sqlx::SqlitePool,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Preserve the platform's consent prompt rather than granting webview capture.
        .on_permission_request(|_, _| tauri::webview::PermissionResponse::Default)
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let data_dir = handle
                    .path()
                    .app_data_dir()
                    .expect("app data directory is always resolvable");
                std::fs::create_dir_all(&data_dir)?;
                let db_path = data_dir.join("shelfledger.db");
                let pool = db::connect(&db_path).await?;
                handle.manage(AppState { pool });
                Ok::<(), Box<dyn std::error::Error>>(())
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::catalog::unarchive_copy,
            commands::catalog::update_edition,
            commands::catalog::update_copy_details,
            commands::catalog::list_work_tags,
            commands::catalog::set_work_tags,
            commands::data::preview_import,
            commands::data::commit_import,
            commands::data::list_libraries,
            commands::data::switch_library,
            commands::catalog::search_works,
            commands::catalog::get_work,
            commands::catalog::add_book,
            commands::catalog::create_work,
            commands::catalog::update_work,
            commands::catalog::create_edition,
            commands::catalog::create_copy,
            commands::catalog::update_copy,
            commands::catalog::archive_copy,
            commands::catalog::find_duplicates,
            commands::catalog::get_library_stats,
            commands::locations::list_locations,
            commands::locations::create_location,
            commands::locations::delete_location,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::metadata::lookup_isbn,
            commands::reading::list_reading_sessions,
            commands::reading::list_reading_sessions_for_work,
            commands::reading::create_reading_session,
            commands::reading::update_reading_session,
            commands::reading::add_reading_progress,
            commands::loans::list_contacts,
            commands::loans::create_contact,
            commands::loans::update_contact,
            commands::loans::list_loans,
            commands::loans::list_active_loans,
            commands::loans::list_overdue_loans,
            commands::loans::lend_copy,
            commands::loans::return_loan,
            commands::finance::add_valuation,
            commands::finance::list_valuations,
            commands::finance::create_listing,
            commands::finance::withdraw_listing,
            commands::finance::list_listings,
            commands::finance::record_disposal,
            commands::finance::list_disposals,
            commands::finance::get_sell_queue,
            commands::finance::set_sell_override,
            commands::finance::get_finance_report,
            commands::finance::get_dashboard,
            commands::data::export_library_csv,
            commands::data::export_library_json,
            commands::data::import_library_json,
            commands::data::preview_goodreads_import,
            commands::data::commit_goodreads_import,
            commands::data::create_backup,
            commands::data::restore_backup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ShelfLedger");
}

#[cfg(test)]
mod audit_regressions;

#[cfg(test)]
mod currency_regressions;
