use crate::{
    domain::market::{self, MarketReport, SaveMarketComparableInput, SaveMarketEstimateInput},
    error::{AppError, AppResult},
    models::ValuationDto,
    AppState,
};
use tauri::State;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn list_market_comparisons(
    state: State<'_, AppState>,
    edition_id: String,
) -> AppResult<MarketReport> {
    market::report(&state.pool, &edition_id).await
}
#[tauri::command]
pub async fn save_market_comparison(
    state: State<'_, AppState>,
    input: SaveMarketComparableInput,
) -> AppResult<()> {
    market::save(&state.pool, &input).await
}
#[tauri::command]
pub async fn set_market_comparison_included(
    state: State<'_, AppState>,
    id: String,
    included: bool,
) -> AppResult<()> {
    market::set_included(&state.pool, &id, included).await
}
#[tauri::command]
pub async fn save_market_estimate(
    state: State<'_, AppState>,
    input: SaveMarketEstimateInput,
) -> AppResult<ValuationDto> {
    market::save_estimate(&state.pool, &input).await
}
#[tauri::command]
pub fn open_marketplace_url(app: tauri::AppHandle, url: String) -> AppResult<()> {
    let url = market::olx_url(&url, false)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Other(e.to_string()))
}
