fn main() {
    println!("cargo:rerun-if-env-changed=SHELFLEDGER_GOOGLE_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=SHELFLEDGER_GOOGLE_CLIENT_SECRET");
    tauri_build::build()
}
