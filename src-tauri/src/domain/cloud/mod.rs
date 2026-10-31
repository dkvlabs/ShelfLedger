//! Opt-in snapshot sync. Remote revisions are immutable; divergent histories
//! require an explicit choice. Tokens and sync links never enter library exports.
pub mod drive;
#[cfg(test)]
mod tests;

use super::backup;
use crate::{
    error::{AppError, AppResult},
    repositories::{
        settings,
        snapshot::{self, Snapshot},
    },
    sqlite_db::SqlitePool,
};
use drive::{Drive, RemoteFile};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub const MAX_CLOUD_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Link {
    pub library_id: String,
    pub library_name: String,
    pub revision: Option<String>,
    pub digest: Option<String>,
    pub last_synced: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Revision {
    format_version: u32,
    library_id: String,
    digest: String,
    snapshot: Snapshot,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudLibrary {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudHead {
    pub id: String,
    pub created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflict {
    pub local_digest: String,
    pub heads: Vec<CloudHead>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudHistory {
    pub local_digest: String,
    pub expected_heads: Vec<String>,
    pub backups: Vec<CloudHead>,
}

pub async fn history(pool: &SqlitePool, root: &Path, drive: &Drive) -> AppResult<CloudHistory> {
    let active = profile(pool).await?;
    let link = load_link(root, &active)?
        .ok_or_else(|| AppError::Validation("choose a cloud library first".into()))?;
    if !libraries(drive)
        .await?
        .iter()
        .any(|l| l.id == link.library_id)
    {
        return Err(AppError::NotFound(
            "cloud library is not in this Google account".into(),
        ));
    }
    let files = drive.list("revision", Some(&link.library_id)).await?;
    let expected_heads = heads(&files)?.into_iter().map(|f| f.id).collect();
    let local_digest = digest(&snapshot::export_for_profile(pool, &active).await?)?;
    let mut backups: Vec<_> = files
        .into_iter()
        .map(|f| CloudHead {
            id: f.id,
            created_at: f.created_time,
        })
        .collect();
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    Ok(CloudHistory {
        local_digest,
        expected_heads,
        backups,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub status: &'static str,
    pub last_synced: Option<String>,
    pub local_backup_path: Option<String>,
    pub conflict: Option<SyncConflict>,
    pub pending_local_changes: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Resolution {
    pub choice: Choice,
    pub local_digest: String,
    pub expected_heads: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Choice {
    KeepDevice,
    UseCloud { revision: String },
}

fn validate_uuid(id: &str) -> AppResult<()> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Err(AppError::Validation("invalid cloud library ID".into()));
    }
    Ok(())
}

pub fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "library name must contain 1–80 characters".into(),
        ));
    }
    Ok(name.to_owned())
}

pub async fn profile(pool: &SqlitePool) -> AppResult<String> {
    let id = settings::get_json(pool, "_active_profile")
        .await?
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "main".into());
    if id != "main" {
        validate_uuid(&id)?;
    }
    Ok(id)
}

fn link_path(root: &Path, profile: &str) -> AppResult<PathBuf> {
    if profile != "main" {
        validate_uuid(profile)?;
    }
    crate::paths::write_path(
        root,
        &root.join("cloud-sync").join(profile).join("link.json"),
    )
}

pub fn load_link(root: &Path, profile: &str) -> AppResult<Option<Link>> {
    let path = link_path(root, profile)?;
    match std::fs::read(path) {
        Ok(bytes) if bytes.len() <= 8192 => {
            let link: Link = serde_json::from_slice(&bytes)?;
            validate_uuid(&link.library_id)?;
            Ok(Some(link))
        }
        Ok(_) => Err(AppError::Validation("invalid sync link".into())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn save_link(root: &Path, profile: &str, link: &Link) -> AppResult<()> {
    validate_uuid(&link.library_id)?;
    let path = link_path(root, profile)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    let temp = path.with_extension(format!("{}.tmp", crate::ids::new_id()));
    std::fs::write(&temp, serde_json::to_vec(link)?)?;
    // rename replaces on Unix; on Windows remove only this disposable checkpoint.
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    std::fs::rename(temp, path)?;
    Ok(())
}

pub fn disconnect(root: &Path, profile: &str) -> AppResult<()> {
    match std::fs::remove_file(link_path(root, profile)?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Preferences/profile selection remain local. Keep insertion order intact:
/// valuation history uses it to break equal observation/creation timestamps.
pub fn portable(mut data: Snapshot) -> Snapshot {
    data.tables.insert("settings".into(), Vec::new());
    data
}

pub fn digest(data: &Snapshot) -> AppResult<String> {
    let mut canonical = portable(data.clone());
    for (table, rows) in &mut canonical.tables {
        if table != "valuation_records" {
            rows.sort_by_cached_key(|row| serde_json::to_string(row).unwrap());
        }
    }
    Ok(backup::sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn empty(data: &Snapshot) -> bool {
    portable(data.clone()).tables.values().all(Vec::is_empty)
}

fn heads(files: &[RemoteFile]) -> AppResult<Vec<RemoteFile>> {
    let ids: BTreeSet<_> = files.iter().map(|f| f.id.clone()).collect();
    if ids.len() != files.len() {
        return Err(AppError::Validation("duplicate cloud revision ID".into()));
    }
    let mut superseded = BTreeSet::new();
    let mut edges = std::collections::BTreeMap::new();
    for f in files {
        let parents = f.parents()?;
        for parent in &parents {
            if parent == &f.id {
                return Err(AppError::Validation(
                    "invalid cloud revision history".into(),
                ));
            }
            superseded.insert(parent.clone());
        }
        edges.insert(
            f.id.clone(),
            parents
                .into_iter()
                .filter(|p| ids.contains(p))
                .collect::<BTreeSet<_>>(),
        );
    }
    // Reject cycles even when a separate valid branch also has a head.
    while !edges.is_empty() {
        let roots: BTreeSet<_> = edges
            .iter()
            .filter(|(_, parents)| parents.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        if roots.is_empty() {
            return Err(AppError::Validation(
                "invalid cloud revision history".into(),
            ));
        }
        edges.retain(|id, parents| {
            parents.retain(|p| !roots.contains(p));
            !roots.contains(id)
        });
    }
    let result: Vec<_> = files
        .iter()
        .filter(|f| !superseded.contains(&f.id))
        .cloned()
        .collect();
    if !files.is_empty() && result.is_empty() {
        return Err(AppError::Validation(
            "invalid cloud revision history".into(),
        ));
    }
    Ok(result)
}

enum Plan {
    Equal,
    Push,
    Pull,
    Conflict,
}
fn plan(local: &str, remote: &str, baseline: Option<&str>, local_empty: bool) -> Plan {
    if local == remote {
        Plan::Equal
    } else if baseline == Some(remote) {
        Plan::Push
    } else if baseline == Some(local) || (baseline.is_none() && local_empty) {
        Plan::Pull
    } else {
        Plan::Conflict
    }
}

async fn validate_snapshot(data: &Snapshot) -> AppResult<()> {
    let pool = crate::db::connect_memory().await?;
    let result = snapshot::apply(&pool, data, true).await;
    pool.close().await;
    result
}

async fn read_revision(drive: &Drive, file: &RemoteFile, library_id: &str) -> AppResult<Revision> {
    let revision: Revision = serde_json::from_slice(&drive.download(&file.id).await?)?;
    if revision.format_version != 1
        || revision.library_id != library_id
        || revision.snapshot.format_version != 4
        || revision.digest != digest(&revision.snapshot)?
        || !revision
            .snapshot
            .tables
            .get("settings")
            .is_some_and(Vec::is_empty)
    {
        return Err(AppError::Validation(
            "invalid or incompatible cloud backup".into(),
        ));
    }
    validate_snapshot(&revision.snapshot).await?;
    Ok(revision)
}

async fn upload_revision(
    drive: &Drive,
    link: &Link,
    data: &Snapshot,
    parents: &[String],
) -> AppResult<String> {
    let snapshot = portable(data.clone());
    validate_snapshot(&snapshot).await?;
    let revision = Revision {
        format_version: 1,
        library_id: link.library_id.clone(),
        digest: digest(&snapshot)?,
        snapshot,
    };
    let bytes = serde_json::to_vec(&revision)?;
    if bytes.len() > MAX_CLOUD_BYTES {
        return Err(AppError::Validation(
            "cloud backup exceeds 64 MiB; use a ZIP backup instead".into(),
        ));
    }
    drive
        .upload(
            "revision",
            &link.library_id,
            &format!("ShelfLedger revision {}", crate::ids::new_id()),
            parents,
            bytes,
        )
        .await
}

pub async fn libraries(drive: &Drive) -> AppResult<Vec<CloudLibrary>> {
    let files = drive.list("library", None).await?;
    let mut libraries = Vec::new();
    let mut ids = BTreeSet::new();
    for f in files {
        let Some(id) = f.app_properties.get("libraryId") else {
            continue;
        };
        validate_uuid(id)?;
        if !ids.insert(id.clone()) {
            return Err(AppError::Validation("duplicate cloud library".into()));
        }
        libraries.push(CloudLibrary {
            id: id.clone(),
            name: f.name,
        });
    }
    libraries.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(libraries)
}

pub async fn link_library(
    pool: &SqlitePool,
    root: &Path,
    drive: &Drive,
    id: &str,
) -> AppResult<()> {
    validate_uuid(id)?;
    let current_profile = profile(pool).await?;
    let library = libraries(drive)
        .await?
        .into_iter()
        .find(|l| l.id == id)
        .ok_or_else(|| AppError::NotFound("cloud library is not in this Google account".into()))?;
    if profile(pool).await? != current_profile {
        return Err(AppError::Conflict("library changed; choose again".into()));
    }
    save_link(
        root,
        &current_profile,
        &Link {
            library_id: library.id,
            library_name: library.name,
            revision: None,
            digest: None,
            last_synced: None,
        },
    )
}

pub async fn create_library(drive: &Drive, name: &str) -> AppResult<CloudLibrary> {
    let name = validate_name(name)?;
    let id = crate::ids::new_id();
    drive
        .upload(
            "library",
            &id,
            &name,
            &[],
            serde_json::to_vec(&serde_json::json!({"formatVersion":1,"libraryId":id}))?,
        )
        .await?;
    Ok(CloudLibrary { id, name })
}

pub async fn sync(
    pool: &SqlitePool,
    root: &Path,
    drive: &Drive,
    resolution: Option<Resolution>,
) -> AppResult<SyncResult> {
    let active_profile = profile(pool).await?;
    sync_for_profile(pool, root, drive, resolution, &active_profile).await
}

/// First backup needs no user-created cloud container. Reuse the only existing
/// backup in this account; require a choice if several libraries are present.
pub async fn start_sync(
    pool: &SqlitePool,
    root: &Path,
    drive: &Drive,
    expected_profile: &str,
    library_id: Option<&str>,
) -> AppResult<SyncResult> {
    if profile(pool).await? != expected_profile {
        return Err(AppError::Conflict("library changed; try again".into()));
    }
    if load_link(root, expected_profile)?.is_none() {
        let available = libraries(drive).await?;
        let selected = match library_id {
            Some(id) => available
                .into_iter()
                .find(|l| l.id == id)
                .ok_or_else(|| AppError::NotFound("backup is not in this Google account".into()))?,
            None if available.is_empty() => create_library(drive, "ShelfLedger backup").await?,
            None if available.len() == 1 => available.into_iter().next().unwrap(),
            None => return Err(AppError::Validation("choose a cloud backup first".into())),
        };
        if profile(pool).await? != expected_profile {
            return Err(AppError::Conflict("library changed; try again".into()));
        }
        save_link(
            root,
            expected_profile,
            &Link {
                library_id: selected.id,
                library_name: selected.name,
                revision: None,
                digest: None,
                last_synced: None,
            },
        )?;
    }
    sync_for_profile(pool, root, drive, None, expected_profile).await
}

async fn sync_for_profile(
    pool: &SqlitePool,
    root: &Path,
    drive: &Drive,
    resolution: Option<Resolution>,
    expected_profile: &str,
) -> AppResult<SyncResult> {
    let active_profile = expected_profile.to_owned();
    if profile(pool).await? != active_profile {
        return Err(AppError::Conflict("library changed; try again".into()));
    }
    let mut link = load_link(root, &active_profile)?
        .ok_or_else(|| AppError::Validation("choose a cloud library first".into()))?;
    // Check account membership even when no revision exists: wrong accounts must
    // never silently receive a newly created backup for an existing link.
    if !libraries(drive)
        .await?
        .iter()
        .any(|l| l.id == link.library_id)
    {
        return Err(AppError::NotFound(
            "cloud library is not in this Google account; reconnect with the original account"
                .into(),
        ));
    }
    let local = snapshot::export_for_profile(pool, &active_profile).await?;
    let local_digest = digest(&local)?;
    let files = drive.list("revision", Some(&link.library_id)).await?;
    let remote_heads = heads(&files)?;
    let parent_ids: Vec<_> = remote_heads.iter().map(|f| f.id.clone()).collect();
    let conflict = || SyncResult {
        status: "conflict",
        last_synced: link.last_synced.clone(),
        local_backup_path: None,
        conflict: Some(SyncConflict {
            local_digest: local_digest.clone(),
            heads: remote_heads
                .iter()
                .map(|f| CloudHead {
                    id: f.id.clone(),
                    created_at: f.created_time.clone(),
                })
                .collect(),
        }),
        pending_local_changes: true,
    };
    let (action, selected) = if let Some(resolution) = resolution {
        if resolution.local_digest != local_digest
            || resolution.expected_heads.iter().collect::<BTreeSet<_>>()
                != parent_ids.iter().collect::<BTreeSet<_>>()
        {
            return Ok(conflict());
        }
        match resolution.choice {
            Choice::KeepDevice => (Plan::Push, None),
            Choice::UseCloud { revision } => {
                let Some(file) = files.iter().find(|f| f.id == revision) else {
                    return Ok(conflict());
                };
                (
                    Plan::Pull,
                    Some((
                        file.id.clone(),
                        read_revision(drive, file, &link.library_id).await?,
                    )),
                )
            }
        }
    } else if remote_heads.is_empty() {
        // A formerly synced remote history disappearing needs explicit recovery.
        if link.revision.is_some() {
            return Err(AppError::Conflict(
                "cloud backups were removed; reconnect or create a new cloud library".into(),
            ));
        }
        (Plan::Push, None)
    } else if remote_heads.len() == 1 {
        let remote = read_revision(drive, &remote_heads[0], &link.library_id).await?;
        (
            plan(
                &local_digest,
                &remote.digest,
                link.digest.as_deref(),
                empty(&local),
            ),
            Some((remote_heads[0].id.clone(), remote)),
        )
    } else {
        return Ok(conflict());
    };
    if matches!(action, Plan::Conflict) {
        return Ok(conflict());
    }
    if profile(pool).await? != active_profile {
        return Err(AppError::Conflict(
            "library changed during sync; try again".into(),
        ));
    }
    let mut local_backup_path = None;
    let (revision, synced_digest) = match action {
        Plan::Push => (
            upload_revision(drive, &link, &local, &parent_ids).await?,
            local_digest.clone(),
        ),
        Plan::Equal => (remote_heads[0].id.clone(), local_digest.clone()),
        Plan::Pull => {
            let (selected_id, remote) = selected.unwrap();
            let path = crate::paths::write_path(
                root,
                &root
                    .join("backups")
                    .join(format!("pre-cloud-sync-{}.zip", crate::ids::new_id())),
            )?;
            backup::create_backup(pool, &root.join("shelfledger.db"), &path).await?;
            // Resolve forks by adding a new revision, keeping every earlier file.
            let revision = if remote_heads.len() > 1
                || remote_heads.first().map(|h| h.id.as_str()) != Some(selected_id.as_str())
            {
                upload_revision(drive, &link, &remote.snapshot, &parent_ids).await?
            } else {
                remote_heads[0].id.clone()
            };
            if profile(pool).await? != active_profile {
                return Err(AppError::Conflict(
                    "library changed during sync; try again".into(),
                ));
            }
            let mut incoming = remote.snapshot;
            incoming
                .tables
                .insert("settings".into(), local.tables["settings"].clone());
            snapshot::apply_if_unchanged(pool, &incoming, &local, &active_profile).await?;
            local_backup_path = Some(path.to_string_lossy().into_owned());
            (revision, remote.digest)
        }
        Plan::Conflict => unreachable!(),
    };
    link.revision = Some(revision);
    link.digest = Some(synced_digest.clone());
    link.last_synced = Some(crate::ids::now_iso());
    save_link(root, &active_profile, &link)?;
    let pending_local_changes = digest(&snapshot::export(pool).await?)? != synced_digest;
    Ok(SyncResult {
        status: "synced",
        last_synced: link.last_synced,
        local_backup_path,
        conflict: None,
        pending_local_changes,
    })
}
