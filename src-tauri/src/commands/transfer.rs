//! Native document pickers keep Android content URIs out of the domain layer.
//! Imports are bounded, staged under app data, and never applied by picking.
//! Export copies can only originate in exports/backups and the destination is
//! obtained from the OS picker inside Rust, never supplied by the webview.
use crate::error::{AppError, AppResult};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::{FsExt, OpenOptions};

const MAX_IMPORT_BYTES: u64 = 512 * 1024 * 1024;

fn validate_extensions(extensions: &[String]) -> AppResult<()> {
    if extensions.is_empty()
        || extensions
            .iter()
            .any(|ext| !matches!(ext.as_str(), "csv" | "json" | "zip"))
    {
        return Err(AppError::Validation(
            "choose CSV, JSON or a ZIP backup".into(),
        ));
    }
    Ok(())
}

pub(crate) fn stage_import(
    root: &Path,
    reader: impl Read,
    extensions: &[String],
    limit: u64,
) -> AppResult<PathBuf> {
    validate_extensions(extensions)?;
    let directory = crate::paths::write_path(root, &root.join("uploads"))?;
    std::fs::create_dir_all(&directory)?;
    let temporary = directory.join(format!("{}.part", crate::ids::new_id()));
    let result = (|| {
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let size = std::io::copy(&mut reader.take(limit + 1), &mut output)?;
        output.sync_all()?;
        drop(output);
        if size == 0 || size > limit {
            return Err(AppError::Validation(
                "import must be nonempty and no larger than 512 MiB".into(),
            ));
        }
        // Content providers need not expose a filename, so classify the staged
        // data rather than treating a content:// URI as a filesystem path.
        let mut input = std::io::BufReader::new(File::open(&temporary)?);
        let mut signature = [0; 4];
        let count = input.read(&mut signature)?;
        let extension = if count == 4 && matches!(&signature, b"PK\x03\x04" | b"PK\x05\x06") {
            "zip"
        } else {
            let first = signature[..count]
                .iter()
                .copied()
                .chain(input.bytes().map_while(Result::ok))
                .find(|byte| {
                    !byte.is_ascii_whitespace() && *byte != 0xef && *byte != 0xbb && *byte != 0xbf
                });
            if matches!(first, Some(b'{' | b'[')) {
                "json"
            } else {
                "csv"
            }
        };
        if !extensions.iter().any(|allowed| allowed == extension) {
            return Err(AppError::Validation(
                "selected file does not match the requested format".into(),
            ));
        }
        let target = temporary.with_extension(extension);
        std::fs::rename(&temporary, &target)?;
        Ok(target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn export_source(root: &Path, requested: &Path) -> AppResult<PathBuf> {
    let root = root.canonicalize()?;
    let source = requested.canonicalize()?;
    if !source.is_file()
        || !["exports", "backups"]
            .iter()
            .any(|dir| source.starts_with(root.join(dir)))
    {
        return Err(AppError::Validation(
            "only generated exports and backups can be saved".into(),
        ));
    }
    Ok(source)
}

#[tauri::command]
pub async fn pick_import_file(
    app: tauri::AppHandle,
    extensions: Vec<String>,
) -> AppResult<Option<String>> {
    validate_extensions(&extensions)?;
    tauri::async_runtime::spawn_blocking(move || {
        let filters = extensions.iter().map(String::as_str).collect::<Vec<_>>();
        let Some(selected) = app
            .dialog()
            .file()
            .add_filter("Library files", &filters)
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let root = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Other(e.to_string()))?;
        let mut options = OpenOptions::new();
        options.read(true);
        let input = app.fs().open(selected, options)?;
        let path = stage_import(&root, input, &extensions, MAX_IMPORT_BYTES)?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?
}

#[tauri::command]
pub async fn save_export(app: tauri::AppHandle, path: String) -> AppResult<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Other(e.to_string()))?;
        let source = export_source(&root, Path::new(&path))?;
        let name = source
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| AppError::Validation("invalid export filename".into()))?;
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("zip");
        let Some(destination) = app
            .dialog()
            .file()
            .set_file_name(name)
            .add_filter("Library file", &[extension])
            .blocking_save_file()
        else {
            return Ok(false);
        };
        // Prevent truncating the source if the user selects that same file.
        if let Ok(destination_path) = destination.clone().into_path() {
            if destination_path.canonicalize().ok().as_ref() == Some(&source) {
                return Ok(true);
            }
        }
        let mut input = File::open(source)?;
        let mut options = OpenOptions::new();
        options.read(false).write(true).create(true).truncate(true);
        let mut output = app.fs().open(destination, options)?;
        std::io::copy(&mut input, &mut output)?;
        output.sync_all()?;
        Ok(true)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stages_named_formats_without_trusting_provider_paths_and_rejects_oversized_data() {
        let root = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir_all(&root).unwrap();
        let formats = vec!["csv".into(), "json".into(), "zip".into()];
        for (bytes, extension) in [
            (b" Title,Author\nBook,Writer".as_slice(), "csv"),
            (b" \n{\"tables\":{}}".as_slice(), "json"),
            (b"\xef\xbb\xbf[1]".as_slice(), "json"),
            (b"PK\x03\x04fixture".as_slice(), "zip"),
        ] {
            let file = stage_import(&root, bytes, &formats, 100).unwrap();
            assert_eq!(file.extension().unwrap(), extension);
            assert_eq!(std::fs::read(file).unwrap(), bytes);
        }
        for (bytes, formats) in [
            (b"".as_slice(), formats.clone()),
            (b"too large".as_slice(), formats.clone()),
            (b"{}".as_slice(), vec!["zip".into()]),
            (b"{}".as_slice(), vec!["../json".into()]),
        ] {
            assert!(stage_import(&root, bytes, &formats, 4).is_err());
        }
        assert!(std::fs::read_dir(root.join("uploads"))
            .unwrap()
            .all(|file| file.unwrap().path().extension().unwrap() != "part"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn only_export_files_can_leave_app_storage() {
        let root = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir_all(root.join("backups")).unwrap();
        std::fs::create_dir_all(root.join("exports")).unwrap();
        for name in [
            "backups/library.zip",
            "exports/library.json",
            "shelfledger.db",
            "exports-secret.json",
        ] {
            std::fs::write(root.join(name), "fixture").unwrap();
        }
        assert!(export_source(&root, &root.join("backups/library.zip")).is_ok());
        assert!(export_source(&root, &root.join("exports/library.json")).is_ok());
        assert!(export_source(&root, &root.join("shelfledger.db")).is_err());
        assert!(export_source(&root, &root.join("exports-secret.json")).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                root.join("shelfledger.db"),
                root.join("exports/escape.json"),
            )
            .unwrap();
            assert!(export_source(&root, &root.join("exports/escape.json")).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
