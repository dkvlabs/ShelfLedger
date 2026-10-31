//! Portability (Phase 4): CSV/JSON export, complete backup archives, and
//! validated restore. Backups use SQLite's `VACUUM INTO` (crash-safe snapshot,
//! not a naive live WAL copy) plus a hashed manifest in a zip archive.

use crate::sqlite_db as sqlx;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};

const BACKUP_MANIFEST_NAME: &str = "manifest.json";
const BACKUP_DB_NAME: &str = "shelfledger.db";
const BACKUP_FORMAT_VERSION: i64 = 1;

// -- Exports ------------------------------------------------------------------

pub type CopyExportRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<String>,
);

/// Flat CSV of every non-archived copy with its work/edition/acquisition.
/// One row per copy; currencies stay per-row and are never summed.
pub fn build_copies_csv(rows: Vec<CopyExportRow>) -> String {
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record([
        "work_title",
        "authors",
        "isbn13",
        "isbn10",
        "publisher",
        "condition_grade",
        "ownership_status",
        "location",
        "acquired_at",
        "store",
        "price_minor",
        "shipping_minor",
        "tax_minor",
        "currency",
    ])
    .expect("csv header");
    for r in rows {
        let (
            work_title,
            authors,
            isbn13,
            isbn10,
            publisher,
            condition_grade,
            ownership_status,
            location,
            acquired_at,
            store,
            price_minor,
            shipping_minor,
            tax_minor,
            currency,
        ) = r;
        let opt = |v: Option<String>| v.unwrap_or_default();
        w.write_record([
            work_title,
            authors,
            isbn13,
            isbn10,
            opt(publisher),
            condition_grade,
            ownership_status,
            opt(location),
            opt(acquired_at),
            opt(store),
            price_minor.map(|v| v.to_string()).unwrap_or_default(),
            shipping_minor.map(|v| v.to_string()).unwrap_or_default(),
            tax_minor.map(|v| v.to_string()).unwrap_or_default(),
            opt(currency),
        ])
        .expect("csv row");
    }
    String::from_utf8(w.into_inner().expect("csv flush")).expect("utf8 csv")
}

// -- Backup -------------------------------------------------------------------

/// Create a backup archive: `VACUUM INTO` snapshot + manifest + hashes, zipped
/// under the caller-supplied output path (a .zip file that must not exist).
pub async fn create_backup(
    pool: &SqlitePool,
    _db_path: &std::path::Path,
    output_path: &std::path::Path,
) -> AppResult<std::path::PathBuf> {
    if output_path.exists() {
        return Err(AppError::Validation(
            "backup file already exists; choose another name".into(),
        ));
    }
    let parent = output_path
        .parent()
        .ok_or_else(|| AppError::Validation("invalid backup path".into()))?;
    std::fs::create_dir_all(parent)?;

    // Crash-safe snapshot into a sibling temp file.
    let snapshot = parent.join(format!("snapshot-{}.db", crate::ids::new_id()));
    let snapshot_str = snapshot
        .to_str()
        .ok_or_else(|| AppError::Validation("invalid snapshot path".into()))?
        .to_string();
    let result = async {
        sqlx::query("VACUUM INTO ?")
            .bind(&snapshot_str)
            .execute(pool)
            .await?;

        // Verify the snapshot opens and passes an integrity check.
        let verify = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&snapshot)
            .read_only(true);
        let verify_pool = sqlx::SqlitePool::connect_with(verify).await?;
        let verification = async {
            let (check,): (String,) = sqlx::query_as("PRAGMA integrity_check")
                .fetch_one(&verify_pool)
                .await?;
            if check != "ok" {
                return Err(AppError::Other(format!(
                    "snapshot failed integrity check: {check}"
                )));
            }
            let (schema_version,): (i64,) =
                sqlx::query_as("SELECT coalesce(MAX(version), 0) FROM _sqlx_migrations")
                    .fetch_one(&verify_pool)
                    .await?;
            Ok::<_, AppError>(schema_version)
        }
        .await;
        // Error paths must also wait for SQLite's worker to release the file.
        verify_pool.close().await;
        let schema_version = verification?;

        let db_bytes = std::fs::read(&snapshot)?;
        let db_hash = sha256_hex(&db_bytes);
        let manifest = BackupManifest {
            format_version: BACKUP_FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: ids_now(),
            schema_version,
            database_sha256: db_hash,
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("serializable manifest");

        // Zip everything up (zip-slip is not applicable: we only write two fixed
        // entry names, never entries read from the archive here).
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        zip.start_file(BACKUP_DB_NAME, options)
            .map_err(|e| AppError::Other(format!("zip error: {e}")))?;
        std::io::Write::write_all(&mut zip, &db_bytes)
            .map_err(|e| AppError::Other(format!("zip write error: {e}")))?;
        zip.start_file(BACKUP_MANIFEST_NAME, options)
            .map_err(|e| AppError::Other(format!("zip error: {e}")))?;
        std::io::Write::write_all(&mut zip, &manifest_bytes)
            .map_err(|e| AppError::Other(format!("zip write error: {e}")))?;
        zip.finish()
            .map_err(|e| AppError::Other(format!("zip finish error: {e}")))?;
        Ok::<_, AppError>(())
    }
    .await;
    let cleanup = crate::file_cleanup::remove_file(&snapshot).await;
    result?;
    cleanup?;
    Ok(output_path.to_path_buf())
}

/// Validate a backup archive against the manifest and database integrity,
/// then transactionally replace records on the live pool, or save a separate
/// selectable profile. The live database/WAL files are never overwritten.
pub async fn restore_archive(
    pool: &SqlitePool,
    live_db_path: &std::path::Path,
    archive_path: &std::path::Path,
    mode: &str,
) -> AppResult<crate::models::RestoreResult> {
    let file = std::fs::File::open(archive_path)
        .map_err(|e| AppError::Validation(format!("could not open backup: {e}")))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| AppError::Validation(format!("could not read backup archive: {e}")))?;

    // Read and validate the manifest first.
    let manifest_bytes = extract_entry(&mut zip, BACKUP_MANIFEST_NAME)?;
    let manifest: BackupManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| AppError::Validation(format!("invalid backup manifest: {e}")))?;
    if manifest.format_version > BACKUP_FORMAT_VERSION {
        return Err(AppError::Validation(format!(
            "backup was made by a newer version (format {} > {})",
            manifest.format_version, BACKUP_FORMAT_VERSION
        )));
    }
    let db_bytes = extract_entry(&mut zip, BACKUP_DB_NAME)?;
    let actual_hash = sha256_hex(&db_bytes);
    if actual_hash != manifest.database_sha256 {
        return Err(AppError::Validation(
            "database hash does not match the manifest; the backup is corrupted".into(),
        ));
    }

    // Extract to a temp file and run migrations + integrity check there.
    if !matches!(mode, "replace" | "separate") {
        return Err(AppError::Validation("unknown restore mode".into()));
    }
    let work_dir = live_db_path
        .parent()
        .ok_or_else(|| AppError::Validation("invalid library path".into()))?
        .join(format!("restore-{}", crate::ids::new_id()));
    std::fs::create_dir_all(&work_dir)?;
    let restored_path = work_dir.join(BACKUP_DB_NAME);
    let result = async {
        std::fs::write(&restored_path, &db_bytes)?;

        let verify_opts = sqlx::sqlite::SqliteConnectOptions::new().filename(&restored_path);
        let verify_pool = sqlx::SqlitePool::connect_with(verify_opts).await?;
        let verification = async {
            let (check,): (String,) = sqlx::query_as("PRAGMA integrity_check")
                .fetch_one(&verify_pool)
                .await?;
            if check != "ok" {
                return Err(AppError::Other(format!(
                    "backup failed integrity check: {check}"
                )));
            }
            // Forward migrations are safe; anything older is upgraded.
            crate::db::MIGRATOR.run(&verify_pool).await?;
            crate::repositories::snapshot::export(&verify_pool).await
        }
        .await;
        verify_pool.close().await;
        verification
    }
    .await;
    let cleanup = crate::file_cleanup::remove_dir_all(&work_dir).await;
    let snapshot = result?;
    // The snapshot is in memory now. Finish staging cleanup before changing
    // the live library, so a cleanup error cannot misreport a completed restore.
    cleanup?;

    let result = match mode {
        "replace" => {
            let safety =
                live_db_path.with_extension(format!("pre-restore-{}.zip", crate::ids::new_id()));
            create_backup(pool, live_db_path, &safety).await?;
            crate::repositories::snapshot::apply(pool, &snapshot, true).await?;
            crate::models::RestoreResult {
                message: format!(
                    "Library restored. Previous library saved at {}.",
                    safety.display()
                ),
                requires_restart: false,
            }
        }
        "separate" => {
            let root = live_db_path.parent().unwrap();
            let dir = crate::paths::write_path(root, &root.join("profiles"))?;
            std::fs::create_dir_all(&dir)?;
            let target = dir.join(format!("{}.json", crate::ids::new_id()));
            std::fs::write(&target, serde_json::to_vec_pretty(&snapshot)?)?;
            crate::models::RestoreResult {
                message: format!(
                    "Restored a separate library. Open it in Settings → Libraries. Saved at {}.",
                    target.display()
                ),
                requires_restart: false,
            }
        }
        other => {
            return Err(AppError::Validation(format!(
                "unknown restore mode {other:?}"
            )));
        }
    };
    Ok(result)
}

fn extract_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> AppResult<Vec<u8>> {
    let mut entry = zip
        .by_name(name)
        .map_err(|_| AppError::Validation(format!("backup archive is missing {name}")))?;
    // Bounded read: a hostile archive cannot balloon memory (512 MiB cap).
    const MAX_ENTRY: u64 = 512 * 1024 * 1024;
    if entry.size() > MAX_ENTRY {
        return Err(AppError::Validation(format!(
            "{name} is unreasonably large"
        )));
    }
    let mut buf = Vec::with_capacity(entry.size() as usize);
    std::io::Read::read_to_end(&mut entry, &mut buf)
        .map_err(|e| AppError::Validation(format!("could not read {name}: {e}")))?;
    Ok(buf)
}

#[derive(Debug, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    format_version: i64,
    app_version: String,
    created_at: String,
    schema_version: i64,
    database_sha256: String,
}

fn ids_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    // Small dependency-free SHA-256 so backups self-verify without adding a
    // crypto crate for one hash.
    let mut h = Sha256::new();
    h.update(bytes);
    hex(&h.finalize())
}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        while !data.is_empty() {
            let take = (64 - self.buffer_len).min(data.len());
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&data[..take]);
            self.buffer_len += take;
            data = &data[take..];
            if self.buffer_len == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffer_len = 0;
            }
        }
    }

    fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len * 8;
        self.update(&[0x80]);
        while self.buffer_len != 56 {
            self.update(&[0]);
        }
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect;

    #[test]
    fn sha_known_vectors() {
        assert_eq!(
            sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[tokio::test]
    async fn backup_and_restore_roundtrip_preserves_data() {
        // Real on-disk library with one work.
        let dir = std::env::temp_dir()
            .join(crate::ids::new_id())
            .join("Library %25 # 文 books");
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let db_path = dir.join("shelfledger.db");
        let pool = connect(&db_path).await.unwrap();
        crate::domain::catalog::create_work(
            &pool,
            &crate::models::CreateWorkInput {
                title: "Backup Test".into(),
                subtitle: None,
                original_language: None,
                description: None,
                author_names: vec!["A".into()],
            },
        )
        .await
        .unwrap();
        pool.close().await;

        // Backup, wipe, restore, verify content.
        let archive = dir.join("backup.zip");
        let pool = connect(&db_path).await.unwrap();
        create_backup(&pool, &db_path, &archive).await.unwrap();
        assert!(std::fs::read_dir(&dir).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("snapshot-")));

        // Corrupt detection: tampered archive must be rejected.
        let tampered = dir.join("tampered.zip");
        let mut bytes = std::fs::read(&archive).unwrap();
        let len = bytes.len();
        bytes[len - 3] ^= 0xFF; // flip a manifest byte
        std::fs::write(&tampered, &bytes).unwrap();
        let err = restore_archive(&pool, &db_path, &tampered, "replace")
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::Validation(_)),
            "expected validation error, got {err:?}"
        );

        // Legit restore restores the work.
        let result = restore_archive(&pool, &db_path, &archive, "replace")
            .await
            .unwrap();
        assert!(!result.requires_restart);
        pool.close().await;
        let pool = connect(&db_path).await.unwrap();
        let summaries = crate::repositories::works::list_work_summaries(&pool)
            .await
            .unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].title, "Backup Test");
        pool.close().await;
        crate::file_cleanup::remove_dir_all(dir.parent().unwrap())
            .await
            .unwrap();
    }

    #[test]
    fn csv_export_escapes_and_formats() {
        let csv = build_copies_csv(vec![(
            "Title, with comma".to_string(),
            "Author \"Q\"".to_string(),
            "9780306406157".to_string(),
            String::new(),
            Some("Pub".to_string()),
            "good".to_string(),
            "owned".to_string(),
            None,
            Some("2026-01-01".to_string()),
            Some("Store".to_string()),
            Some(1250),
            Some(100),
            None,
            Some("RON".to_string()),
        )]);
        assert!(csv.contains("\"Title, with comma\""));
        assert!(csv.contains("\"Author \"\"Q\"\"\""));
        assert!(csv.contains("1250"));
    }

    #[tokio::test]
    async fn failed_backup_verification_closes_connections_and_removes_snapshot() {
        let dir = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("library.db");
        let pool = connect(&db_path).await.unwrap();
        // Integrity succeeds, but querying the backup's schema version fails.
        sqlx::query("DROP TABLE _sqlx_migrations")
            .execute(&pool)
            .await
            .unwrap();
        let archive = dir.join("backup.zip");
        assert!(create_backup(&pool, &db_path, &archive).await.is_err());
        assert!(!archive.exists());
        assert!(std::fs::read_dir(&dir).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("snapshot-")));
        pool.close().await;
        crate::file_cleanup::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn rejected_restore_removes_temporary_database_without_changing_live_data() {
        let dir = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("library.db");
        let pool = connect(&db_path).await.unwrap();
        sqlx::query("INSERT INTO works(id,title,created_at,updated_at) VALUES('legacy','Preserve on failure','x','x')")
            .execute(&pool).await.unwrap();
        let archive = dir.join("backup.zip");
        create_backup(&pool, &db_path, &archive).await.unwrap();
        let error = restore_archive(&pool, &db_path, &archive, "replace")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("invalid ISO date: x"));
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT title FROM works WHERE id='legacy'")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "Preserve on failure"
        );
        assert!(std::fs::read_dir(&dir).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("restore-")));
        pool.close().await;
        crate::file_cleanup::remove_dir_all(&dir).await.unwrap();
    }
}
