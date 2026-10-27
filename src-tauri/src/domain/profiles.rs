//! Separate restored libraries, selected explicitly. The open database stays
//! stable; switching snapshots is atomic and the previous library is saved.
use crate::sqlite_db as sqlx;
use crate::{
    error::{AppError, AppResult},
    repositories::snapshot,
};
use sqlx::SqlitePool;
use std::path::Path;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub active: bool,
}

pub async fn list(pool: &SqlitePool, root: &Path) -> AppResult<Vec<Profile>> {
    let active = crate::repositories::settings::get_json(pool, "_active_profile")
        .await?
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "main".into());
    let mut out = vec![Profile {
        id: "main".into(),
        name: "Main library".into(),
        active: active == "main",
    }];
    let dir = root.join("profiles");
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if let Some(id) = path
                .file_stem()
                .and_then(|v| v.to_str())
                .filter(|v| uuid::Uuid::parse_str(v).is_ok())
            {
                out.push(Profile {
                    id: id.into(),
                    name: format!("Restored library · {}", &id[..8]),
                    active: active == id,
                });
            }
        }
    }
    Ok(out)
}

pub async fn switch(pool: &SqlitePool, root: &Path, id: &str) -> AppResult<()> {
    let profiles = list(pool, root).await?;
    if !profiles.iter().any(|p| p.id == id) {
        return Err(AppError::Validation("unknown library".into()));
    }
    let active = &profiles
        .iter()
        .find(|p| p.active)
        .ok_or_else(|| AppError::Other("active library missing".into()))?
        .id;
    if active == id {
        return Ok(());
    }
    let dir = crate::paths::write_path(root, &root.join("profiles"))?;
    std::fs::create_dir_all(&dir)?;
    let snapshot = snapshot::export(pool).await?;
    let temp = dir.join(format!("{}.tmp", crate::ids::new_id()));
    std::fs::write(&temp, serde_json::to_vec_pretty(&snapshot)?)?;
    std::fs::rename(temp, dir.join(format!("{active}.json")))?;
    let mut target: snapshot::Snapshot =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{id}.json")))?)?;
    let settings = target
        .tables
        .get_mut("settings")
        .ok_or_else(|| AppError::Validation("incomplete library".into()))?;
    settings.retain(|v| v.get("key").and_then(|v| v.as_str()) != Some("_active_profile"));
    settings.push(std::collections::BTreeMap::from([
        ("key".into(), serde_json::json!("_active_profile")),
        (
            "value_json".into(),
            serde_json::json!(serde_json::to_string(id)?),
        ),
    ]));
    snapshot::apply(pool, &target, true).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn switching_back_keeps_changes_to_each_library() {
        let root = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir_all(root.join("profiles")).unwrap();
        let pool = crate::db::connect_memory().await.unwrap();
        let empty = snapshot::export(&pool).await.unwrap();
        let id = crate::ids::new_id();
        std::fs::write(
            root.join(format!("profiles/{id}.json")),
            serde_json::to_vec(&empty).unwrap(),
        )
        .unwrap();
        sqlx::query("INSERT INTO settings VALUES ('marker','1')")
            .execute(&pool)
            .await
            .unwrap();
        switch(&pool, &root, &id).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM settings WHERE key='marker'")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        // Restoring portable content must keep this installation's selected profile.
        assert!(!snapshot::export(&pool).await.unwrap().tables["settings"]
            .iter()
            .any(|r| r.get("key").and_then(serde_json::Value::as_str) == Some("_active_profile")));
        snapshot::apply(&pool, &empty, true).await.unwrap();
        assert!(list(&pool, &root)
            .await
            .unwrap()
            .iter()
            .any(|p| p.id == id && p.active));
        switch(&pool, &root, "main").await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM settings WHERE key='marker'")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
