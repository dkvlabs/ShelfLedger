use crate::error::{AppError, AppResult};
use std::path::{Component, Path, PathBuf};

/// Validate every existing ancestor before creating directories. This rejects
/// traversal and symlink escapes without creating anything outside app data.
pub fn write_path(root: &Path, requested: &Path) -> AppResult<PathBuf> {
    let reject = || AppError::Validation("output must stay inside the app data directory".into());
    let original_root = root;
    let root = root.canonicalize()?;
    let target = if let Ok(relative) = requested.strip_prefix(original_root) {
        root.join(relative)
    } else if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        root.join(requested)
    };
    if target
        .components()
        .any(|c| matches!(c, Component::ParentDir))
        || !target.starts_with(&root)
    {
        return Err(reject());
    }
    let mut ancestor = target.as_path();
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(reject)?;
    }
    if !ancestor.canonicalize()?.starts_with(&root) {
        return Err(reject());
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal_and_symlink_escape() {
        let root = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir_all(&root).unwrap();
        assert!(write_path(&root, &root.join("backups/a.zip")).is_ok());
        assert!(write_path(&root, &root.join("../outside.zip")).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(std::env::temp_dir(), root.join("escape")).unwrap();
            assert!(write_path(&root, &root.join("escape/a.zip")).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
