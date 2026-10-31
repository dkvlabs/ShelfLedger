//! Minimal Drive transport, scoped to this app's hidden appDataFolder.
use super::MAX_CLOUD_BYTES;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, time::Duration};

pub const SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub created_time: String,
    #[serde(default)]
    pub app_properties: BTreeMap<String, String>,
}

impl RemoteFile {
    pub fn parents(&self) -> AppResult<Vec<String>> {
        let parents: Vec<String> = serde_json::from_str(&self.description)?;
        if parents.len() > 1000 {
            return Err(AppError::Validation("cloud history is too large".into()));
        }
        for parent in &parents {
            validate_file_id(parent)?;
        }
        Ok(parents)
    }
}

fn validate_file_id(id: &str) -> AppResult<()> {
    if id.is_empty()
        || id.len() > 200
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(AppError::Validation("invalid Google Drive file ID".into()));
    }
    Ok(())
}

pub fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("ShelfLedger/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| AppError::Network("could not initialize cloud connection".into()))
}

pub async fn response_bytes(mut response: reqwest::Response, limit: usize) -> AppResult<Vec<u8>> {
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::Network(
            match status.as_u16() {
                401 => "Google sign-in expired. Sign in again.",
                403 => {
                    "Google Drive denied access. Check account permissions and available storage."
                }
                429 => "Google Drive is busy. Try again later.",
                _ => "Google Drive request failed. Check your connection and try again.",
            }
            .into(),
        ));
    }
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(AppError::Validation("cloud response is too large".into()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        AppError::Network("cloud download interrupted; no local data was changed".into())
    })? {
        if bytes.len() + chunk.len() > limit {
            return Err(AppError::Validation("cloud response is too large".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub struct Drive {
    client: reqwest::Client,
    token: String,
    api: String,
    upload_api: String,
}

impl Drive {
    pub fn new(token: String) -> AppResult<Self> {
        Ok(Self {
            client: client()?,
            token,
            api: "https://www.googleapis.com/drive/v3".into(),
            upload_api: "https://www.googleapis.com/upload/drive/v3".into(),
        })
    }

    #[cfg(test)]
    pub fn test_server(endpoint: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            token: "test-only-token".into(),
            api: endpoint.clone(),
            upload_api: endpoint,
        }
    }

    pub async fn list(&self, kind: &str, library_id: Option<&str>) -> AppResult<Vec<RemoteFile>> {
        if !matches!(kind, "library" | "revision") {
            return Err(AppError::Validation("unknown cloud file kind".into()));
        }
        let mut q = format!(
            "trashed = false and appProperties has {{ key='shelfledgerKind' and value='{kind}' }}"
        );
        if let Some(id) = library_id {
            super::validate_uuid(id)?;
            q.push_str(&format!(
                " and appProperties has {{ key='libraryId' and value='{id}' }}"
            ));
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Page {
            files: Vec<RemoteFile>,
            next_page_token: Option<String>,
        }
        let mut next = String::new();
        let mut files = Vec::new();
        let mut pages = std::collections::BTreeSet::new();
        loop {
            if pages.len() >= 100 || !pages.insert(next.clone()) {
                return Err(AppError::Validation("invalid cloud pagination".into()));
            }
            let response = self
                .client
                .get(format!("{}/files", self.api))
                .bearer_auth(&self.token)
                .query(&[
                    ("spaces", "appDataFolder"),
                    ("q", &q),
                    ("pageSize", "1000"),
                    ("pageToken", &next),
                    (
                        "fields",
                        "nextPageToken,files(id,name,description,createdTime,appProperties)",
                    ),
                ])
                .send()
                .await
                .map_err(|_| AppError::Network("could not reach Google Drive".into()))?;
            let page: Page =
                serde_json::from_slice(&response_bytes(response, 4 * 1024 * 1024).await?)?;
            for file in &page.files {
                validate_file_id(&file.id)?;
                if file
                    .app_properties
                    .get("shelfledgerKind")
                    .map(String::as_str)
                    != Some(kind)
                    || library_id.is_some_and(|id| {
                        file.app_properties.get("libraryId").map(String::as_str) != Some(id)
                    })
                    || chrono::DateTime::parse_from_rfc3339(&file.created_time).is_err()
                {
                    return Err(AppError::Validation("invalid cloud file metadata".into()));
                }
            }
            files.extend(page.files);
            if files.len() > 1000 {
                return Err(AppError::Validation("this cloud library has more than 1000 backups; export it and create a new cloud library".into()));
            }
            match page.next_page_token {
                Some(token) if !token.is_empty() && token.len() <= 8192 => next = token,
                Some(_) => return Err(AppError::Validation("invalid cloud pagination".into())),
                None => return Ok(files),
            }
        }
    }

    pub async fn download(&self, id: &str) -> AppResult<Vec<u8>> {
        validate_file_id(id)?;
        let response = self
            .client
            .get(format!("{}/files/{id}", self.api))
            .bearer_auth(&self.token)
            .query(&[("alt", "media")])
            .send()
            .await
            .map_err(|_| AppError::Network("could not download the cloud backup".into()))?;
        response_bytes(response, MAX_CLOUD_BYTES).await
    }

    pub async fn upload(
        &self,
        kind: &str,
        library_id: &str,
        name: &str,
        parents: &[String],
        bytes: Vec<u8>,
    ) -> AppResult<String> {
        super::validate_uuid(library_id)?;
        if bytes.len() > MAX_CLOUD_BYTES || parents.len() > 1000 {
            return Err(AppError::Validation("cloud backup is too large".into()));
        }
        for parent in parents {
            validate_file_id(parent)?;
        }
        let boundary = format!("shelfledger-{}", crate::ids::new_id());
        let metadata = json!({"name":name,"mimeType":"application/json","parents":["appDataFolder"],
            "description":serde_json::to_string(parents)?,"appProperties":{"shelfledgerKind":kind,"libraryId":library_id}});
        let mut body = format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/json\r\n\r\n").into_bytes();
        body.extend(bytes);
        body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
        let response = self
            .client
            .post(format!("{}/files", self.upload_api))
            .bearer_auth(&self.token)
            .query(&[("uploadType", "multipart"), ("fields", "id")])
            .header(
                "Content-Type",
                format!("multipart/related; boundary={boundary}"),
            )
            .body(body)
            .send()
            .await
            .map_err(|_| {
                AppError::Network(
                    "cloud upload interrupted; existing backups were preserved".into(),
                )
            })?;
        #[derive(Deserialize)]
        struct Created {
            id: String,
        }
        let result: Created = serde_json::from_slice(&response_bytes(response, 64 * 1024).await?)?;
        validate_file_id(&result.id)?;
        Ok(result.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bounded_downloads_refuse_redirects_errors_and_large_responses() {
        use axum::{
            http::{header, StatusCode},
            routing::get,
            Router,
        };
        let router = Router::new()
            .route("/large", get(|| async { "x".repeat(100) }))
            .route("/denied", get(|| async { StatusCode::FORBIDDEN }))
            .route(
                "/redirect",
                get(|| async {
                    (
                        StatusCode::FOUND,
                        [(header::LOCATION, "https://example.com")],
                    )
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        for path in ["large", "denied", "redirect"] {
            let response = client.get(format!("{url}/{path}")).send().await.unwrap();
            assert!(response_bytes(response, 32).await.is_err());
        }
        assert!(validate_file_id("../../escape").is_err());
        server.abort();
    }
}
