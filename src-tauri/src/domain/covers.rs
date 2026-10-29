//! Local, edition-specific covers. Only an explicit fetch contacts a provider.
use crate::{
    error::{AppError, AppResult},
    repositories::covers,
    sqlite_db::SqlitePool,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::{io::Cursor, time::Duration};

pub const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;
const MAX_STORED_BYTES: usize = 2 * 1024 * 1024;
const PREFIX: &str = "data:image/jpeg;base64,";

fn invalid() -> AppError {
    AppError::Validation("Choose a JPEG, PNG or WebP image up to 10 MiB and 32 megapixels.".into())
}

fn decode(bytes: &[u8]) -> AppResult<image::DynamicImage> {
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err(invalid());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| invalid())?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)
    ) {
        return Err(invalid());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| invalid())?;
    let (w, h) = decoder.dimensions();
    if w == 0
        || h == 0
        || u64::from(w) * u64::from(h) > 32_000_000
        || decoder.total_bytes() > 128 * 1024 * 1024
    {
        return Err(invalid());
    }
    let orientation = decoder.orientation().map_err(|_| invalid())?;
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(|_| invalid())?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn normalize(bytes: &[u8]) -> AppResult<String> {
    let image = decode(bytes)?;
    let image = if image.width() > 1200 || image.height() > 1200 {
        image.thumbnail(1200, 1200)
    } else {
        image
    }
    .to_rgba8();
    // Flatten transparency onto white; a fresh encoder strips EXIF/GPS metadata.
    let rgb = image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let p = image.get_pixel(x, y).0;
        let alpha = u32::from(p[3]);
        image::Rgb([0, 1, 2].map(|i| ((u32::from(p[i]) * alpha + 255 * (255 - alpha)) / 255) as u8))
    });
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 82)
        .encode_image(&rgb)
        .map_err(|_| invalid())?;
    if jpeg.len() > MAX_STORED_BYTES {
        return Err(invalid());
    }
    Ok(format!("{PREFIX}{}", STANDARD.encode(jpeg)))
}

pub(crate) fn validate_stored(data_url: &str) -> AppResult<()> {
    let raw = data_url.strip_prefix(PREFIX).ok_or_else(invalid)?;
    if raw.len() > MAX_STORED_BYTES.div_ceil(3) * 4 {
        return Err(invalid());
    }
    let bytes = STANDARD.decode(raw).map_err(|_| invalid())?;
    if bytes.len() > MAX_STORED_BYTES {
        return Err(invalid());
    }
    if image::guess_format(&bytes).ok() != Some(ImageFormat::Jpeg) {
        return Err(invalid());
    }
    let image = decode(&bytes)?;
    if image.width() > 1200 || image.height() > 1200 {
        return Err(invalid());
    }
    Ok(())
}

pub async fn save_photo(
    pool: &SqlitePool,
    edition_id: &str,
    data_url: String,
) -> AppResult<String> {
    covers::edition_isbn(pool, edition_id).await?;
    let encoded = tokio::task::spawn_blocking(move || {
        if data_url.len() > MAX_INPUT_BYTES.div_ceil(3) * 4 + 32 {
            return Err(invalid());
        }
        let (prefix, payload) = data_url.split_once(',').ok_or_else(invalid)?;
        if !matches!(
            prefix,
            "data:image/jpeg;base64" | "data:image/png;base64" | "data:image/webp;base64"
        ) {
            return Err(invalid());
        }
        let bytes = STANDARD.decode(payload).map_err(|_| invalid())?;
        normalize(&bytes)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))??;
    covers::save(pool, edition_id, &encoded, "photo", None).await
}

fn allowed_cover_url(url: &reqwest::Url) -> bool {
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return false;
    }
    match url.host_str() {
        Some("covers.openlibrary.org") => url.path().starts_with("/b/"),
        Some("archive.org") => {
            url.path().starts_with("/download/olcovers") && url.path().ends_with(".jpg")
        }
        Some(host) if host.starts_with("ia") && host.ends_with(".archive.org") => {
            url.path() == "/view_archive.php"
                && url.query_pairs().any(|(k, v)| {
                    k == "archive" && v.contains("/items/olcovers") && v.ends_with(".zip")
                })
                && url
                    .query_pairs()
                    .any(|(k, v)| k == "file" && v.ends_with(".jpg") && !v.contains(['/', '\\']))
        }
        _ => false,
    }
}

async fn fetch_image(isbn: &str) -> AppResult<Option<Vec<u8>>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!(
            "ShelfLedger/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/dkvlabs/ShelfLedger)"
        ))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 4 || !allowed_cover_url(attempt.url()) {
                attempt.error("cover redirect outside provider")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let url = format!("https://covers.openlibrary.org/b/isbn/{isbn}-L.jpg?default=false");
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    response = response
        .error_for_status()
        .map_err(|e| AppError::Network(e.to_string()))?;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_INPUT_BYTES as u64)
    {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?
    {
        if bytes.len() + chunk.len() > MAX_INPUT_BYTES {
            return Err(invalid());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Some(bytes))
}

pub async fn fetch(pool: &SqlitePool, edition_id: &str) -> AppResult<Option<String>> {
    fetch_with_provider(
        pool,
        edition_id,
        |isbn| async move { fetch_image(&isbn).await },
    )
    .await
}

async fn fetch_with_provider<F, Fut>(
    pool: &SqlitePool,
    edition_id: &str,
    provider: F,
) -> AppResult<Option<String>>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = AppResult<Option<Vec<u8>>>>,
{
    let raw = covers::edition_isbn(pool, edition_id)
        .await?
        .ok_or_else(|| AppError::Validation("An ISBN is required to fetch a cover.".into()))?;
    let isbn = crate::isbn::parse(&raw)
        .and_then(|id| id.isbn13)
        .ok_or_else(invalid)?;
    crate::metadata::require_online(pool).await?;
    let Some(bytes) = provider(isbn).await? else {
        return Ok(None);
    };
    let encoded = tokio::task::spawn_blocking(move || normalize(&bytes))
        .await
        .map_err(|e| AppError::Other(e.to_string()))??;
    Ok(Some(
        covers::save(pool, edition_id, &encoded, "openlibrary", Some(&raw)).await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{backup, catalog},
        repositories::{settings, snapshot},
        sqlite_db as sqlx,
    };
    use serde_json::json;

    fn image_bytes(format: ImageFormat, w: u32, h: u32) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(w, h)
            .write_to(&mut bytes, format)
            .unwrap();
        bytes.into_inner()
    }
    async fn book(pool: &SqlitePool) -> crate::models::AddBookResult {
        catalog::create_book(
            pool,
            &serde_json::from_value(json!({
                "title":"Cover test", "authorNames":[], "isbn13":"9780385533225"
            }))
            .unwrap(),
        )
        .await
        .unwrap()
    }
    fn photo() -> String {
        format!(
            "data:image/png;base64,{}",
            STANDARD.encode(image_bytes(ImageFormat::Png, 40, 60))
        )
    }

    #[test]
    fn supported_images_are_bounded_and_normalized_and_active_content_is_rejected() {
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
            let stored = normalize(&image_bytes(format, 1600, 800)).unwrap();
            validate_stored(&stored).unwrap();
            let bytes = STANDARD
                .decode(stored.strip_prefix(PREFIX).unwrap())
                .unwrap();
            let decoded = image::load_from_memory(&bytes).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (1200, 600));
        }
        for bytes in [
            b"<svg onload='alert(1)'>".to_vec(),
            vec![0; MAX_INPUT_BYTES + 1],
            image_bytes(ImageFormat::Png, 8193, 1),
        ] {
            assert!(normalize(&bytes).is_err());
        }
        assert!(validate_stored("https://evil.test/image.jpg").is_err());
        assert!(validate_stored("data:image/jpeg;base64,bm90IGFuIGltYWdl").is_err());
    }

    #[test]
    fn phone_orientation_is_applied_and_exif_is_not_saved() {
        let jpeg = image_bytes(ImageFormat::Jpeg, 40, 20);
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut oriented = vec![0xff, 0xd8, 0xff, 0xe1, 0, 34];
        oriented.extend_from_slice(exif);
        oriented.extend_from_slice(&jpeg[2..]);
        let normalized = normalize(&oriented).unwrap();
        let saved = STANDARD
            .decode(normalized.strip_prefix(PREFIX).unwrap())
            .unwrap();
        let image = image::load_from_memory(&saved).unwrap();
        assert_eq!((image.width(), image.height()), (20, 40));
        assert!(!saved.windows(4).any(|bytes| bytes == b"Exif"));
    }

    #[tokio::test]
    async fn save_replace_remove_and_failed_fetch_preserve_the_right_image() {
        let pool = crate::db::connect_memory().await.unwrap();
        let book = book(&pool).await;
        let path = save_photo(&pool, &book.edition_id, photo()).await.unwrap();
        let saved = covers::get(&pool, &path).await.unwrap().unwrap();
        assert_eq!(saved.source, "photo");
        assert!(save_photo(
            &pool,
            &book.edition_id,
            "data:image/svg+xml;base64,AA==".into()
        )
        .await
        .is_err());
        assert!(save_photo(&pool, "missing", photo()).await.is_err());
        assert!(covers::get(&pool, "../../secret").await.unwrap().is_none());
        for invalid_path in [
            format!("../{path}"),
            format!("{path}/../secret"),
            path.replace("covers/", "covers/../"),
            format!("{path}?query=1"),
            "covers/not-a-uuid.jpg".into(),
        ] {
            assert!(covers::get(&pool, &invalid_path).await.unwrap().is_none());
        }
        assert!(
            fetch_with_provider(&pool, &book.edition_id, |isbn| async move {
                assert_eq!(isbn, "9780385533225");
                Ok(None)
            })
            .await
            .unwrap()
            .is_none()
        );
        assert!(fetch_with_provider(&pool, &book.edition_id, |_| async {
            Err(AppError::Network("offline".into()))
        })
        .await
        .is_err());
        assert_eq!(
            covers::get(&pool, &path).await.unwrap().unwrap().data_url,
            saved.data_url
        );
        settings::set_json(&pool, "metadata.lookup_enabled", &json!(false))
            .await
            .unwrap();
        assert!(matches!(
            fetch_with_provider(&pool, &book.edition_id, |_| async {
                panic!("network must not be polled")
            })
            .await,
            Err(AppError::MetadataDisabled)
        ));
        settings::set_json(&pool, "metadata.lookup_enabled", &json!(true))
            .await
            .unwrap();
        let next = fetch_with_provider(&pool, &book.edition_id, |_| async {
            Ok(Some(image_bytes(ImageFormat::Png, 60, 40)))
        })
        .await
        .unwrap()
        .unwrap();
        assert_ne!(path, next);
        assert!(covers::get(&pool, &path).await.unwrap().is_none());
        assert_eq!(
            covers::get(&pool, &next).await.unwrap().unwrap().source,
            "openlibrary"
        );
        let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM edition_covers")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
        covers::remove(&pool, &book.edition_id).await.unwrap();
        assert!(covers::get(&pool, &next).await.unwrap().is_none());
        assert!(catalog::get_work_detail(&pool, &book.work_id)
            .await
            .unwrap()
            .editions[0]
            .cover_path
            .is_none());
    }

    #[tokio::test]
    async fn json_and_zip_transfer_include_images_and_reject_corrupted_image_rows_atomically() {
        let root = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir_all(&root).unwrap();
        let source_path = root.join("source.db");
        let source = crate::db::connect(&source_path).await.unwrap();
        let book = book(&source).await;
        let path = save_photo(&source, &book.edition_id, photo())
            .await
            .unwrap();
        let saved = covers::get(&source, &path).await.unwrap().unwrap().data_url;
        let mut exported = snapshot::export(&source).await.unwrap();
        assert_eq!(exported.format_version, 4);
        let target_path = root.join("target.db");
        let target = crate::db::connect(&target_path).await.unwrap();
        snapshot::apply(&target, &exported, true).await.unwrap();
        assert_eq!(
            covers::get(&target, &path).await.unwrap().unwrap().data_url,
            saved
        );
        exported.tables.get_mut("edition_covers").unwrap()[0]
            .insert("data_url".into(), json!("data:image/svg+xml;base64,AA=="));
        assert!(snapshot::apply(&target, &exported, true).await.is_err());
        assert_eq!(
            covers::get(&target, &path).await.unwrap().unwrap().data_url,
            saved
        );
        let mut invalid = snapshot::export(&source).await.unwrap();
        invalid.tables.get_mut("editions").unwrap()[0]
            .insert("cover_path".into(), json!("covers/missing.jpg"));
        assert!(snapshot::apply(&target, &invalid, true).await.is_err());
        let archive = root.join("library.zip");
        backup::create_backup(&source, &source_path, &archive)
            .await
            .unwrap();
        covers::remove(&target, &book.edition_id).await.unwrap();
        backup::restore_archive(&target, &target_path, &archive, "replace")
            .await
            .unwrap();
        assert_eq!(
            covers::get(&target, &path).await.unwrap().unwrap().data_url,
            saved
        );
        // Real old JSONs have no cover table. Replacing clears new covers too.
        covers::remove(&source, &book.edition_id).await.unwrap();
        let mut legacy = snapshot::export(&source).await.unwrap();
        legacy.format_version = 3;
        legacy.tables.remove("edition_covers");
        snapshot::apply(&target, &legacy, true).await.unwrap();
        assert!(covers::get(&target, &path).await.unwrap().is_none());
        source.close().await;
        target.close().await;
        crate::file_cleanup::remove_dir_all(&root).await.unwrap();
    }

    #[tokio::test]
    async fn isbn_edits_during_fetch_cannot_attach_a_cover_to_the_wrong_edition() {
        let pool = crate::db::connect_memory().await.unwrap();
        let book = book(&pool).await;
        let saved_path = save_photo(&pool, &book.edition_id, photo()).await.unwrap();
        let changed = pool.clone();
        let id = book.edition_id.clone();
        let result = fetch_with_provider(&pool, &book.edition_id, |_| async move {
            sqlx::query(
                "UPDATE editions SET isbn13='9786067882629', isbn10='6067882620' WHERE id=?1",
            )
            .bind(&id)
            .execute(&changed)
            .await
            .unwrap();
            Ok(Some(image_bytes(ImageFormat::Png, 40, 60)))
        })
        .await;
        assert!(matches!(result, Err(AppError::Conflict(_))));
        assert!(covers::get(&pool, &saved_path).await.unwrap().is_some());
        sqlx::query("UPDATE editions SET isbn13=NULL,isbn10=NULL WHERE id=?1")
            .bind(&book.edition_id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(
            fetch_with_provider(&pool, &book.edition_id, |_| async {
                panic!("no ISBN must not fetch")
            })
            .await,
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn redirects_only_follow_cover_provider_hosts_and_image_paths() {
        for url in [
            "http://covers.openlibrary.org/b/isbn/a.jpg",
            "https://covers.openlibrary.org.evil.test/b/a.jpg",
            "https://archive.org/download/secret.jpg",
            "https://ia800406.us.archive.org/admin",
            "https://user@covers.openlibrary.org/b/a.jpg",
        ] {
            assert!(
                !allowed_cover_url(&reqwest::Url::parse(url).unwrap()),
                "{url}"
            );
        }
        assert!(allowed_cover_url(
            &reqwest::Url::parse(
                "https://covers.openlibrary.org/b/isbn/9780385533225-L.jpg?default=false"
            )
            .unwrap()
        ));
        assert!(allowed_cover_url(
            &reqwest::Url::parse(
                "https://archive.org/download/olcovers642/olcovers642-L.zip/6429406-L.jpg"
            )
            .unwrap()
        ));
        assert!(allowed_cover_url(&reqwest::Url::parse("https://ia800406.us.archive.org/view_archive.php?archive=/23/items/olcovers642/olcovers642-L.zip&file=6429406-L.jpg").unwrap()));
    }
}
