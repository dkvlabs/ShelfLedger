//! Explicit, read-only release checks. No library data or credentials are sent.
use std::time::Duration;

use crate::error::{AppError, AppResult};
use semver::Version;
use serde::{Deserialize, Serialize};

const RELEASE_API: &str = "https://api.github.com/repos/dkvlabs/ShelfLedger/releases/latest";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub platform: &'static str,
}

pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub download_available: bool,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
}

/// Only fixed project-owned destinations can be opened by the webview.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePage {
    Release,
    AndroidBuilds,
}

pub fn page_url(page: UpdatePage) -> &'static str {
    match page {
        UpdatePage::Release => "https://github.com/dkvlabs/ShelfLedger/releases/latest",
        UpdatePage::AndroidBuilds => {
            "https://github.com/dkvlabs/ShelfLedger/actions/workflows/android.yml"
        }
    }
}

fn release_check(body: &[u8], current: &str, platform: &str, arch: &str) -> AppResult<UpdateCheck> {
    let release: Release = serde_json::from_slice(body)
        .map_err(|_| AppError::Network("invalid release response".into()))?;
    let latest = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )
    .map_err(|_| AppError::Network("invalid release version".into()))?;
    if release.draft || release.prerelease || !latest.pre.is_empty() {
        return Err(AppError::Network(
            "expected a published stable release".into(),
        ));
    }
    let current = Version::parse(current)
        .map_err(|_| AppError::Other("invalid installed application version".into()))?;
    let arch = match arch {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    };
    let prefix = format!("ShelfLedger_{latest}_{platform}-{arch}");
    let download_available = release.assets.iter().any(|asset| {
        if asset.size == 0 {
            return false;
        }
        if platform == "android" {
            let name = asset.name.to_ascii_lowercase();
            // The currently supported Android distribution target is ARM64.
            return arch == "arm64"
                && name.contains("shelfledger")
                && name.contains("android")
                && name.contains("arm64")
                && name.ends_with(".apk");
        }
        let suffix = asset.name.strip_prefix(&prefix).unwrap_or("");
        match platform {
            "macos" => suffix == ".dmg",
            "windows" => suffix == ".exe" || suffix == ".msi",
            "linux" => suffix == ".AppImage" || suffix == ".deb",
            _ => false,
        }
    });
    Ok(UpdateCheck {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        // Build metadata is not part of SemVer precedence.
        update_available: latest.cmp_precedence(&current).is_gt(),
        download_available,
    })
}

async fn fetch_release(client: &reqwest::Client, endpoint: &str) -> AppResult<Vec<u8>> {
    let mut response = client
        .get(endpoint)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| AppError::Network(format!("update check failed: {e}")))?;
    if !response.status().is_success() {
        return Err(AppError::Network(format!(
            "update service returned HTTP {}",
            response.status().as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
    {
        return Err(AppError::Network("release response is too large".into()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| AppError::Network(format!("update response failed: {e}")))?
    {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(AppError::Network("release response is too large".into()));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub async fn check_for_updates() -> AppResult<UpdateCheck> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("ShelfLedger/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let body = fetch_release(&client, RELEASE_API).await?;
    release_check(
        &body,
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release(tag: &str) -> serde_json::Value {
        json!({"tag_name":tag,"draft":false,"prerelease":false,"assets":[
            {"name":"ShelfLedger_0.10.0_macos-arm64.dmg","size":100},
            {"name":"ShelfLedger_0.10.0_windows-x64.exe","size":100},
            {"name":"ShelfLedger_0.10.0_linux-arm64.deb","size":100}
        ]})
    }

    fn check(
        value: &serde_json::Value,
        current: &str,
        os: &str,
        arch: &str,
    ) -> AppResult<UpdateCheck> {
        release_check(&serde_json::to_vec(value).unwrap(), current, os, arch)
    }

    #[test]
    fn uses_semver_precedence_and_never_downgrades() {
        let release = release("v0.10.0");
        for (current, available) in [
            ("0.9.0", true),
            ("0.10.0", false),
            ("0.11.0", false),
            ("0.10.0-beta.1", true),
            ("0.10.0+local", false),
        ] {
            assert_eq!(
                check(&release, current, "macos", "aarch64")
                    .unwrap()
                    .update_available,
                available
            );
        }
    }

    #[test]
    fn only_reports_downloads_for_the_running_platform_and_architecture() {
        let mut release = release("v0.10.0");
        for (os, arch, available) in [
            ("macos", "aarch64", true),
            ("macos", "x86_64", false),
            ("linux", "aarch64", true),
            ("windows", "x86_64", true),
            ("android", "aarch64", false),
            ("linux", "x86_64", false),
        ] {
            assert_eq!(
                check(&release, "0.9.0", os, arch)
                    .unwrap()
                    .download_available,
                available
            );
        }
        release["assets"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"ShelfLedger-android-arm64-release.apk","size":100}));
        assert!(
            check(&release, "0.9.0", "android", "aarch64")
                .unwrap()
                .download_available
        );
        assert!(
            !check(&release, "0.9.0", "android", "x86_64")
                .unwrap()
                .download_available
        );
        release["assets"][3]["size"] = json!(0);
        assert!(
            !check(&release, "0.9.0", "android", "aarch64")
                .unwrap()
                .download_available
        );
    }

    #[test]
    fn rejects_malformed_draft_and_prerelease_results() {
        assert!(release_check(b"not JSON", "0.4.0", "android", "aarch64").is_err());
        for tag in [
            "not-a-version",
            "v0.5.0-beta.1",
            "v999999999999999999999999.0.0",
        ] {
            assert!(check(&release(tag), "0.4.0", "android", "aarch64").is_err());
        }
        for flag in ["draft", "prerelease"] {
            let mut value = release("v0.10.0");
            value[flag] = json!(true);
            assert!(check(&value, "0.4.0", "android", "aarch64").is_err());
        }
    }

    #[test]
    fn app_version_and_open_destinations_are_local_and_fixed() {
        assert_eq!(app_info().version, env!("CARGO_PKG_VERSION"));
        assert_eq!(app_info().platform, std::env::consts::OS);
        assert_eq!(
            page_url(UpdatePage::Release),
            "https://github.com/dkvlabs/ShelfLedger/releases/latest"
        );
        assert!(serde_json::from_str::<UpdatePage>("\"https://example.com\"").is_err());
    }

    #[tokio::test]
    async fn bounded_http_rejects_errors_redirects_and_oversized_bodies() {
        use axum::{
            http::{header, StatusCode},
            routing::get,
            Router,
        };
        let router = Router::new()
            .route("/ok", get(|| async { "{\"tag_name\":\"v0.10.0\"}" }))
            .route("/limited", get(|| async { StatusCode::FORBIDDEN }))
            .route(
                "/redirect",
                get(|| async {
                    (
                        StatusCode::FOUND,
                        [(header::LOCATION, "https://example.com/")],
                    )
                }),
            )
            .route(
                "/large",
                get(|| async { "x".repeat(MAX_RESPONSE_BYTES + 1) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        assert_eq!(
            fetch_release(&client, &format!("http://{address}/ok"))
                .await
                .unwrap(),
            b"{\"tag_name\":\"v0.10.0\"}"
        );
        for (path, message) in [
            ("limited", "HTTP 403"),
            ("redirect", "HTTP 302"),
            ("large", "too large"),
        ] {
            let error = fetch_release(&client, &format!("http://{address}/{path}"))
                .await
                .unwrap_err();
            assert_eq!(error.kind(), "network");
            assert!(error.to_string().contains(message));
        }
        server.abort();
    }
}
