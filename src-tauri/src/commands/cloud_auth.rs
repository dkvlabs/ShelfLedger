//! Native authorization only. Access tokens stay in memory and never cross IPC.
#[cfg(not(target_os = "android"))]
use crate::domain::cloud::drive;
use crate::error::{AppError, AppResult};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct AccessToken {
    pub value: String,
    pub expires: Instant,
}

#[derive(Default)]
pub struct CloudRuntime {
    pub token: Mutex<Option<AccessToken>>,
    pub operation: Mutex<()>,
}

impl CloudRuntime {
    pub async fn access_token(&self) -> AppResult<String> {
        self.token
            .lock()
            .await
            .as_ref()
            .filter(|t| t.expires > Instant::now())
            .map(|t| t.value.clone())
            .ok_or_else(|| {
                AppError::Network("Sign in to Google Drive to sync this library.".into())
            })
    }
}

pub fn configured() -> bool {
    #[cfg(target_os = "android")]
    {
        true
    }
    #[cfg(not(target_os = "android"))]
    {
        desktop_config().is_some()
    }
}

#[cfg(not(target_os = "android"))]
fn desktop_config() -> Option<(String, String)> {
    let id = std::env::var("SHELFLEDGER_GOOGLE_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("SHELFLEDGER_GOOGLE_CLIENT_ID").map(str::to_owned))?;
    let secret = std::env::var("SHELFLEDGER_GOOGLE_CLIENT_SECRET")
        .ok()
        .or_else(|| option_env!("SHELFLEDGER_GOOGLE_CLIENT_SECRET").map(str::to_owned))
        .unwrap_or_default();
    (id.ends_with(".apps.googleusercontent.com")
        && id.len() < 256
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c)))
    .then_some((id, secret))
}

#[cfg(target_os = "android")]
pub struct AndroidAuth(pub tauri::plugin::PluginHandle<tauri::Wry>);

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("shelfledger-cloud")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle =
                    _api.register_android_plugin("io.github.shelfledger.app", "GoogleDrivePlugin")?;
                _app.manage(AndroidAuth(handle));
            }
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
pub async fn authorize(app: tauri::AppHandle) -> AppResult<AccessToken> {
    use serde::Deserialize;
    use tauri::Manager;
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Authorized {
        access_token: String,
    }
    let token: Authorized = tauri::async_runtime::spawn_blocking(move || {
        app.state::<AndroidAuth>().0.run_mobile_plugin("authorizeDrive", serde_json::json!({}))
    }).await.map_err(|_| AppError::Other("Google sign-in was interrupted".into()))?
        .map_err(|_| AppError::Network("Google sign-in failed or was cancelled. Check this app's Google registration and try again.".into()))?;
    if token.access_token.is_empty() {
        return Err(AppError::Network(
            "Google did not grant Drive access".into(),
        ));
    }
    Ok(AccessToken {
        value: token.access_token,
        expires: Instant::now() + Duration::from_secs(3000),
    })
}

#[cfg(not(target_os = "android"))]
fn pkce_challenge(verifier: &str) -> String {
    use base64::Engine;
    let hex = crate::domain::backup::sha256_hex(verifier.as_bytes());
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(not(target_os = "android"))]
pub async fn authorize(app: tauri::AppHandle) -> AppResult<AccessToken> {
    use axum::{
        extract::{Query, State},
        routing::get,
        Router,
    };
    use serde::Deserialize;
    use std::sync::Arc;
    use tauri_plugin_opener::OpenerExt;
    let (client_id, client_secret) = desktop_config().ok_or_else(|| {
        AppError::Validation("Google Drive sign-in is not configured in this build.".into())
    })?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let redirect_uri = format!("http://{}/oauth/callback", listener.local_addr()?);
    let state = crate::ids::new_id();
    let verifier = format!(
        "{}{}",
        crate::ids::new_id().replace('-', ""),
        crate::ids::new_id().replace('-', "")
    );
    let (sender, receiver) = tokio::sync::oneshot::channel::<Result<String, ()>>();
    struct Callback {
        state: String,
        sender: Mutex<Option<tokio::sync::oneshot::Sender<Result<String, ()>>>>,
    }
    #[derive(Deserialize)]
    struct Reply {
        state: Option<String>,
        code: Option<String>,
        error: Option<String>,
    }
    async fn callback(
        State(data): State<Arc<Callback>>,
        Query(reply): Query<Reply>,
    ) -> (axum::http::StatusCode, &'static str) {
        if reply.state.as_deref() != Some(&data.state) {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                "Invalid authorization response.",
            );
        }
        let result = match (reply.code, reply.error) {
            (Some(code), None) if !code.is_empty() && code.len() <= 8192 => Ok(code),
            _ => Err(()),
        };
        if let Some(sender) = data.sender.lock().await.take() {
            let _ = sender.send(result);
        }
        (
            axum::http::StatusCode::OK,
            "Return to ShelfLedger. You can close this tab.",
        )
    }
    let router = Router::new()
        .route("/oauth/callback", get(callback))
        .with_state(Arc::new(Callback {
            state: state.clone(),
            sender: Mutex::new(Some(sender)),
        }));
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    // Abort on every return path, including cancelled commands and failed opening.
    struct Stop(tokio::task::JoinHandle<()>);
    impl Drop for Stop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _stop = Stop(server);
    let mut url = reqwest::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    url.query_pairs_mut().extend_pairs(&[
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("response_type", "code"),
        ("scope", drive::SCOPE),
        ("state", state.as_str()),
        ("code_challenge", pkce_challenge(&verifier).as_str()),
        ("code_challenge_method", "S256"),
        ("access_type", "online"),
        ("prompt", "select_account"),
    ]);
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|_| AppError::Other("Could not open Google sign-in in your browser.".into()))?;
    let code = tokio::time::timeout(Duration::from_secs(180), receiver)
        .await
        .map_err(|_| AppError::Network("Google sign-in timed out. Try again.".into()))?
        .map_err(|_| AppError::Network("Google sign-in was interrupted.".into()))?
        .map_err(|_| AppError::Network("Google sign-in was cancelled.".into()))?;
    #[derive(Deserialize)]
    struct Token {
        access_token: String,
        expires_in: u64,
        scope: Option<String>,
        token_type: String,
    }
    let response = drive::client()?
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|_| AppError::Network("Could not finish Google sign-in.".into()))?;
    let token: Token = serde_json::from_slice(&drive::response_bytes(response, 64 * 1024).await?)?;
    if token.access_token.is_empty()
        || !token.token_type.eq_ignore_ascii_case("bearer")
        || token.expires_in < 60
        || token
            .scope
            .as_ref()
            .is_some_and(|s| !s.split_whitespace().any(|s| s == drive::SCOPE))
    {
        return Err(AppError::Network(
            "Google did not grant the requested Drive access.".into(),
        ));
    }
    Ok(AccessToken {
        value: token.access_token,
        expires: Instant::now() + Duration::from_secs(token.expires_in.min(3600) - 30),
    })
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    #[test]
    fn pkce_matches_the_rfc7636_s256_vector() {
        assert_eq!(
            super::pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
