use tauri::State;

use crate::domain;
use crate::error::AppResult;
use crate::models::{
    ContactDto, CreateContactInput, LendCopyInput, LoanDto, ReturnCopyInput, UpdateContactInput,
};
use crate::AppState;

#[tauri::command]
pub async fn list_contacts(state: State<'_, AppState>) -> AppResult<Vec<ContactDto>> {
    domain::loans::list_contacts(&state.pool).await
}

#[tauri::command]
pub async fn create_contact(
    state: State<'_, AppState>,
    input: CreateContactInput,
) -> AppResult<ContactDto> {
    domain::loans::create_contact(&state.pool, &input).await
}

#[tauri::command]
pub async fn update_contact(
    state: State<'_, AppState>,
    id: String,
    input: UpdateContactInput,
) -> AppResult<ContactDto> {
    domain::loans::update_contact(&state.pool, &id, &input).await
}

#[tauri::command]
pub async fn list_loans(
    state: State<'_, AppState>,
    local_today: String,
) -> AppResult<Vec<LoanDto>> {
    domain::loans::list_loans(&state.pool, &local_today).await
}

#[tauri::command]
pub async fn list_active_loans(
    state: State<'_, AppState>,
    local_today: String,
) -> AppResult<Vec<LoanDto>> {
    domain::loans::list_active_loans(&state.pool, &local_today).await
}

#[tauri::command]
pub async fn list_overdue_loans(
    state: State<'_, AppState>,
    local_today: String,
) -> AppResult<Vec<LoanDto>> {
    domain::loans::list_overdue_loans(&state.pool, &local_today).await
}

#[tauri::command]
pub async fn lend_copy(state: State<'_, AppState>, input: LendCopyInput) -> AppResult<LoanDto> {
    domain::loans::lend_copy(&state.pool, &input).await
}

#[tauri::command]
pub async fn return_loan(
    state: State<'_, AppState>,
    loan_id: String,
    input: ReturnCopyInput,
) -> AppResult<LoanDto> {
    domain::loans::return_copy(&state.pool, &loan_id, &input).await
}
