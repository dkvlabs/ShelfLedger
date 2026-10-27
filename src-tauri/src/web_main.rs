//! Entry point for the web version: the same local Rust core as the desktop
//! app, served to a browser over loopback HTTP. No cloud, no accounts — the
//! library database stays on this machine.
//!
//! Environment:
//!   SHELFLEDGER_DATA_DIR  data directory (default `./shelfledger-data`)
//!   SHELFLEDGER_WEB_DIST  built frontend (default `./dist` when present)
//!   SHELFLEDGER_WEB_PORT  port (default 8787)

use std::path::PathBuf;

use shelfledger_lib::web;

#[tokio::main]
async fn main() {
    let data_dir = std::env::var("SHELFLEDGER_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("shelfledger-data"));
    let dist = std::env::var("SHELFLEDGER_WEB_DIST")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.is_dir())
        .or_else(|| {
            let local = PathBuf::from("dist");
            local.is_dir().then_some(local)
        });
    let port: u16 = std::env::var("SHELFLEDGER_WEB_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8787);

    if let Err(e) = web::serve(data_dir, dist, port).await {
        eprintln!("shelfledger-web: {e}");
        std::process::exit(1);
    }
}
