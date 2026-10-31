use super::cloud_auth::{self, CloudRuntime};

use crate::{
    domain::cloud::{self, drive::Drive, CloudLibrary, Link, Resolution, SyncResult},
    error::{AppError, AppResult},
    AppState,
};
use serde::Serialize;
use tauri::{Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    pub native_available: bool,
    pub configured: bool,
    pub signed_in: bool,
    pub link: Option<Link>,
}

fn root(app: &tauri::AppHandle) -> AppResult<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub async fn get_cloud_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, CloudRuntime>,
) -> AppResult<CloudStatus> {
    let profile = cloud::profile(&state.pool).await?;
    Ok(CloudStatus {
        native_available: true,
        configured: cloud_auth::configured(),
        signed_in: runtime.access_token().await.is_ok(),
        link: cloud::load_link(&root(&app)?, &profile)?,
    })
}

#[tauri::command]
pub async fn connect_google_drive(
    app: tauri::AppHandle,
    runtime: State<'_, CloudRuntime>,
) -> AppResult<Vec<CloudLibrary>> {
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| AppError::Conflict("another cloud operation is running".into()))?;
    let token = cloud_auth::authorize(app).await?;
    let libraries = cloud::libraries(&Drive::new(token.value.clone())?).await?;
    *runtime.token.lock().await = Some(token);
    Ok(libraries)
}

#[tauri::command]
pub async fn list_cloud_libraries(
    runtime: State<'_, CloudRuntime>,
) -> AppResult<Vec<CloudLibrary>> {
    cloud::libraries(&Drive::new(runtime.access_token().await?)?).await
}

#[tauri::command]
pub async fn create_cloud_library(
    name: String,
    runtime: State<'_, CloudRuntime>,
) -> AppResult<CloudLibrary> {
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| AppError::Conflict("another cloud operation is running".into()))?;
    cloud::create_library(&Drive::new(runtime.access_token().await?)?, &name).await
}

#[tauri::command]
pub async fn link_cloud_library(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, CloudRuntime>,
    library_id: String,
) -> AppResult<()> {
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| AppError::Conflict("another cloud operation is running".into()))?;
    cloud::link_library(
        &state.pool,
        &root(&app)?,
        &Drive::new(runtime.access_token().await?)?,
        &library_id,
    )
    .await
}

#[tauri::command]
pub async fn sync_cloud_library(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, CloudRuntime>,
    resolution: Option<Resolution>,
) -> AppResult<SyncResult> {
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| AppError::Conflict("another cloud operation is running".into()))?;
    cloud::sync(
        &state.pool,
        &root(&app)?,
        &Drive::new(runtime.access_token().await?)?,
        resolution,
    )
    .await
}

#[tauri::command]
pub async fn disconnect_cloud_library(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, CloudRuntime>,
) -> AppResult<()> {
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| AppError::Conflict("another cloud operation is running".into()))?;
    cloud::disconnect(&root(&app)?, &cloud::profile(&state.pool).await?)?;
    *runtime.token.lock().await = None;
    Ok(())
}

#[tauri::command]
pub async fn list_cloud_backups(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, CloudRuntime>,
) -> AppResult<cloud::CloudHistory> {
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| AppError::Conflict("another cloud operation is running".into()))?;
    cloud::history(
        &state.pool,
        &root(&app)?,
        &Drive::new(runtime.access_token().await?)?,
    )
    .await
}
