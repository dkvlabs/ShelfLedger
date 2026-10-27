use crate::sqlite_db as sqlx;
use tauri::State;

use crate::domain;
use crate::error::{AppError, AppResult};
use crate::models::{
    AddValuationInput, CreateListingInput, DisposalDto, FinanceReport, RecordDisposalInput,
    SaleListingDto, SellQueueItem, ValuationDto,
};
use crate::AppState;

#[tauri::command]
pub async fn add_valuation(
    state: State<'_, AppState>,
    input: AddValuationInput,
) -> AppResult<ValuationDto> {
    domain::finance::add_valuation(&state.pool, &input).await
}

#[tauri::command]
pub async fn list_valuations(
    state: State<'_, AppState>,
    copy_id: String,
) -> AppResult<Vec<ValuationDto>> {
    domain::finance::list_valuations(&state.pool, &copy_id).await
}

#[tauri::command]
pub async fn create_listing(
    state: State<'_, AppState>,
    input: CreateListingInput,
) -> AppResult<SaleListingDto> {
    domain::finance::create_listing(&state.pool, &input).await
}

#[tauri::command]
pub async fn withdraw_listing(state: State<'_, AppState>, listing_id: String) -> AppResult<()> {
    domain::finance::withdraw_listing(&state.pool, &listing_id).await
}

#[tauri::command]
pub async fn list_listings(
    state: State<'_, AppState>,
    copy_id: String,
) -> AppResult<Vec<SaleListingDto>> {
    domain::finance::list_listings(&state.pool, &copy_id).await
}

#[tauri::command]
pub async fn record_disposal(
    state: State<'_, AppState>,
    input: RecordDisposalInput,
) -> AppResult<DisposalDto> {
    domain::finance::record_disposal(&state.pool, &input).await
}

#[tauri::command]
pub async fn list_disposals(
    state: State<'_, AppState>,
    copy_id: String,
) -> AppResult<Vec<DisposalDto>> {
    domain::finance::list_disposals(&state.pool, &copy_id).await
}

#[tauri::command]
pub async fn get_sell_queue(state: State<'_, AppState>) -> AppResult<Vec<SellQueueItem>> {
    domain::finance::sell_queue(&state.pool).await
}

/// Shared by the Tauri command and the web dispatcher (`web::dispatch`).
pub async fn set_sell_override_impl(
    pool: &sqlx::SqlitePool,
    copy_id: &str,
    override_value: Option<&str>,
) -> AppResult<()> {
    if let Some(v) = override_value {
        if !matches!(v, "keep" | "sell" | "donate") {
            return Err(AppError::Validation(format!("unknown override {v:?}")));
        }
    }
    sqlx::query(
        "UPDATE copies SET sell_review_override = ?2, updated_at = ?3 \
         WHERE id = ?1 AND archived_at IS NULL",
    )
    .bind(copy_id)
    .bind(override_value)
    .bind(crate::ids::now_iso())
    .execute(pool)
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn set_sell_override(
    state: State<'_, AppState>,
    copy_id: String,
    override_value: Option<String>,
) -> AppResult<()> {
    set_sell_override_impl(&state.pool, &copy_id, override_value.as_deref()).await
}

#[tauri::command]
pub async fn get_finance_report(state: State<'_, AppState>) -> AppResult<FinanceReport> {
    domain::finance::finance_report(&state.pool).await
}

#[tauri::command]
pub async fn get_dashboard(
    state: State<'_, AppState>,
    local_today: String,
) -> AppResult<crate::models::DashboardDto> {
    domain::dashboard::get_dashboard(&state.pool, &local_today).await
}
