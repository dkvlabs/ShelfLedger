use super::*;
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Default)]
struct Store {
    files: Vec<(RemoteFile, Vec<u8>)>,
    fail_upload: bool,
    corrupt: bool,
}
struct Server {
    task: tokio::task::JoinHandle<()>,
    store: Arc<Mutex<Store>>,
    url: String,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn server() -> Server {
    type Shared = Arc<Mutex<Store>>;
    async fn list(
        State(store): State<Shared>,
        Query(q): Query<HashMap<String, String>>,
        headers: HeaderMap,
    ) -> Json<serde_json::Value> {
        assert_eq!(headers["authorization"], "Bearer test-only-token");
        assert_eq!(q["spaces"], "appDataFolder");
        let query = &q["q"];
        let files: Vec<_> = store
            .lock()
            .await
            .files
            .iter()
            .filter(|(f, _)| {
                query.contains(&format!("value='{}'", f.app_properties["shelfledgerKind"]))
                    && (!query.contains("key='libraryId'")
                        || query.contains(&format!("value='{}'", f.app_properties["libraryId"])))
            })
            .map(|(f, _)| f.clone())
            .collect();
        Json(serde_json::json!({"files":files}))
    }
    async fn download(
        State(store): State<Shared>,
        axum::extract::Path(id): axum::extract::Path<String>,
    ) -> (StatusCode, Vec<u8>) {
        let store = store.lock().await;
        if store.corrupt {
            return (StatusCode::OK, b"invalid JSON".to_vec());
        }
        store
            .files
            .iter()
            .find(|(f, _)| f.id == id)
            .map(|(_, b)| (StatusCode::OK, b.clone()))
            .unwrap_or((StatusCode::NOT_FOUND, Vec::new()))
    }
    async fn upload(
        State(store): State<Shared>,
        headers: HeaderMap,
        body: Bytes,
    ) -> (StatusCode, Json<serde_json::Value>) {
        assert_eq!(headers["authorization"], "Bearer test-only-token");
        let mut store = store.lock().await;
        if store.fail_upload {
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({})));
        }
        let boundary = headers["content-type"]
            .to_str()
            .unwrap()
            .split("boundary=")
            .nth(1)
            .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        let parts: Vec<_> = body.split(&format!("--{boundary}")).collect();
        let metadata: serde_json::Value =
            serde_json::from_str(parts[1].split_once("\r\n\r\n").unwrap().1.trim()).unwrap();
        assert_eq!(metadata["parents"], serde_json::json!(["appDataFolder"]));
        let bytes = parts[2]
            .split_once("\r\n\r\n")
            .unwrap()
            .1
            .trim()
            .as_bytes()
            .to_vec();
        let id = format!("file_{}", store.files.len() + 1);
        let file = RemoteFile {
            id: id.clone(),
            name: metadata["name"].as_str().unwrap().into(),
            description: metadata["description"].as_str().unwrap().into(),
            created_time: crate::ids::now_iso(),
            app_properties: serde_json::from_value(metadata["appProperties"].clone()).unwrap(),
        };
        store.files.push((file, bytes));
        (StatusCode::OK, Json(serde_json::json!({"id":id})))
    }
    let store = Arc::new(Mutex::new(Store::default()));
    let router = Router::new()
        .route("/files", get(list).post(upload))
        .route("/files/{id}", get(download))
        .with_state(store.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Server { task, store, url }
}

async fn device() -> (PathBuf, SqlitePool) {
    let root = std::env::temp_dir().join(format!("shelfledger-cloud-{}", crate::ids::new_id()));
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let pool = crate::db::connect(&root.join("shelfledger.db"))
        .await
        .unwrap();
    (root, pool)
}
async fn work(pool: &SqlitePool, title: &str) {
    crate::domain::catalog::create_work(
        pool,
        &crate::models::CreateWorkInput {
            title: title.into(),
            subtitle: None,
            description: None,
            original_language: None,
            author_names: vec![],
        },
    )
    .await
    .unwrap();
}
async fn dispose(root: PathBuf, pool: SqlitePool) {
    pool.close().await;
    crate::file_cleanup::remove_dir_all(&root).await.unwrap();
}

#[tokio::test]
async fn first_sync_creates_a_personal_backup_and_another_device_finds_it() {
    let server = server().await;
    let drive = Drive::test_server(server.url.clone());
    let (a, pa) = device().await;
    let (b, pb) = device().await;
    work(&pa, "Personal book").await;
    start_sync(&pa, &a, &drive, "main", None).await.unwrap();
    assert_eq!(libraries(&drive).await.unwrap().len(), 1);
    start_sync(&pb, &b, &drive, "main", None).await.unwrap();
    assert_eq!(
        snapshot::export(&pb).await.unwrap().tables["works"].len(),
        1
    );
    assert_eq!(
        load_link(&a, "main").unwrap().unwrap().library_id,
        load_link(&b, "main").unwrap().unwrap().library_id
    );
    let file_count = server.store.lock().await.files.len();
    assert!(start_sync(&pb, &b, &drive, &crate::ids::new_id(), None)
        .await
        .is_err());
    assert_eq!(server.store.lock().await.files.len(), file_count);
    dispose(a, pa).await;
    dispose(b, pb).await;
}

#[tokio::test]
async fn multiple_backups_require_a_choice_and_other_accounts_never_receive_a_linked_library() {
    let account_a = server().await;
    let account_b = server().await;
    let drive_a = Drive::test_server(account_a.url.clone());
    let drive_b = Drive::test_server(account_b.url.clone());
    let first = create_library(&drive_a, "First").await.unwrap();
    create_library(&drive_a, "Second").await.unwrap();
    let (a, pa) = device().await;
    work(&pa, "Belongs to account A").await;
    assert!(start_sync(&pa, &a, &drive_a, "main", None).await.is_err());
    assert!(load_link(&a, "main").unwrap().is_none());
    start_sync(&pa, &a, &drive_a, "main", Some(&first.id))
        .await
        .unwrap();
    assert!(start_sync(&pa, &a, &drive_b, "main", None).await.is_err());
    assert!(account_b.store.lock().await.files.is_empty());
    dispose(a, pa).await;
}

#[tokio::test]
async fn two_devices_push_pull_reconnect_offline_and_preserve_local_preferences() {
    let server = server().await;
    let drive = Drive::test_server(server.url.clone());
    let library = create_library(&drive, "Our books").await.unwrap();
    let (a, pa) = device().await;
    let (b, pb) = device().await;
    link_library(&pa, &a, &drive, &library.id).await.unwrap();
    link_library(&pb, &b, &drive, &library.id).await.unwrap();
    settings::set_json(&pa, "preferences", &serde_json::json!({"currency":"EUR"}))
        .await
        .unwrap();
    settings::set_json(&pb, "preferences", &serde_json::json!({"currency":"RON"}))
        .await
        .unwrap();
    work(&pa, "First device").await;
    assert_eq!(sync(&pa, &a, &drive, None).await.unwrap().status, "synced");
    let pulled = sync(&pb, &b, &drive, None).await.unwrap();
    assert!(Path::new(pulled.local_backup_path.as_ref().unwrap()).is_file());
    assert_eq!(
        digest(&snapshot::export(&pa).await.unwrap()).unwrap(),
        digest(&snapshot::export(&pb).await.unwrap()).unwrap()
    );
    assert_eq!(
        settings::get_json(&pb, "preferences")
            .await
            .unwrap()
            .unwrap()["currency"],
        "RON"
    );
    work(&pb, "Second device").await;
    sync(&pb, &b, &drive, None).await.unwrap();
    sync(&pa, &a, &drive, None).await.unwrap();
    assert_eq!(
        snapshot::export(&pa).await.unwrap().tables["works"].len(),
        2
    );
    let count = server.store.lock().await.files.len();
    sync(&pa, &a, &drive, None).await.unwrap();
    assert_eq!(
        server.store.lock().await.files.len(),
        count,
        "unchanged sync must not add redundant backups"
    );
    assert!(load_link(&a, "main")
        .unwrap()
        .unwrap()
        .last_synced
        .is_some());
    assert!(
        !serde_json::to_string(&snapshot::export(&pa).await.unwrap())
            .unwrap()
            .contains("cloud-sync")
    );
    server.task.abort();
    let before = snapshot::export(&pa).await.unwrap();
    assert!(sync(&pa, &a, &drive, None).await.is_err());
    assert_eq!(snapshot::export(&pa).await.unwrap(), before);
    dispose(a, pa).await;
    dispose(b, pb).await;
}

#[tokio::test]
async fn divergent_edits_require_a_fresh_choice_and_keep_a_recoverable_device_backup() {
    let server = server().await;
    let drive = Drive::test_server(server.url.clone());
    let library = create_library(&drive, "Shared").await.unwrap();
    let (a, pa) = device().await;
    let (b, pb) = device().await;
    for (root, pool) in [(&a, &pa), (&b, &pb)] {
        link_library(pool, root, &drive, &library.id).await.unwrap();
    }
    work(&pa, "Initial").await;
    sync(&pa, &a, &drive, None).await.unwrap();
    sync(&pb, &b, &drive, None).await.unwrap();
    work(&pa, "Remote edit").await;
    sync(&pa, &a, &drive, None).await.unwrap();
    work(&pb, "Local edit").await;
    let before = snapshot::export(&pb).await.unwrap();
    let conflict = sync(&pb, &b, &drive, None).await.unwrap().conflict.unwrap();
    assert_eq!(snapshot::export(&pb).await.unwrap(), before);
    let stale = Resolution {
        choice: Choice::KeepDevice,
        local_digest: "stale".into(),
        expected_heads: conflict.heads.iter().map(|h| h.id.clone()).collect(),
    };
    assert_eq!(
        sync(&pb, &b, &drive, Some(stale)).await.unwrap().status,
        "conflict"
    );
    let result = sync(
        &pb,
        &b,
        &drive,
        Some(Resolution {
            choice: Choice::UseCloud {
                revision: conflict.heads[0].id.clone(),
            },
            local_digest: conflict.local_digest,
            expected_heads: conflict.heads.iter().map(|h| h.id.clone()).collect(),
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot::export(&pb).await.unwrap().tables["works"].len(),
        2
    );
    let archive = result.local_backup_path.unwrap();
    backup::restore_archive(
        &pb,
        &b.join("shelfledger.db"),
        Path::new(&archive),
        "replace",
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot::export(&pb).await.unwrap(),
        before,
        "the losing device version must remain recoverable"
    );
    dispose(a, pa).await;
    dispose(b, pb).await;
}

#[tokio::test]
async fn simultaneous_uploads_preserve_both_heads_until_explicit_resolution() {
    let server = server().await;
    let drive = Drive::test_server(server.url.clone());
    let library = create_library(&drive, "Fork").await.unwrap();
    let (a, pa) = device().await;
    link_library(&pa, &a, &drive, &library.id).await.unwrap();
    work(&pa, "Original").await;
    sync(&pa, &a, &drive, None).await.unwrap();
    let link = load_link(&a, "main").unwrap().unwrap();
    let baseline = snapshot::export(&pa).await.unwrap();
    let parents = vec![link.revision.clone().unwrap()];
    upload_revision(&drive, &link, &baseline, &parents)
        .await
        .unwrap();
    work(&pa, "Offline edit").await;
    upload_revision(
        &drive,
        &link,
        &snapshot::export(&pa).await.unwrap(),
        &parents,
    )
    .await
    .unwrap();
    let conflict = sync(&pa, &a, &drive, None).await.unwrap().conflict.unwrap();
    assert_eq!(conflict.heads.len(), 2);
    let before_count = server.store.lock().await.files.len();
    sync(
        &pa,
        &a,
        &drive,
        Some(Resolution {
            choice: Choice::KeepDevice,
            local_digest: conflict.local_digest,
            expected_heads: conflict.heads.iter().map(|h| h.id.clone()).collect(),
        }),
    )
    .await
    .unwrap();
    assert_eq!(server.store.lock().await.files.len(), before_count + 1);
    assert_eq!(
        heads(&drive.list("revision", Some(&library.id)).await.unwrap())
            .unwrap()
            .len(),
        1
    );
    let history = history(&pa, &a, &drive).await.unwrap();
    assert_eq!(history.backups.len(), 4);
    let result = sync(
        &pa,
        &a,
        &drive,
        Some(Resolution {
            choice: Choice::UseCloud {
                revision: parents[0].clone(),
            },
            local_digest: history.local_digest,
            expected_heads: history.expected_heads,
        }),
    )
    .await
    .unwrap();
    assert!(result.local_backup_path.is_some());
    assert_eq!(
        snapshot::export(&pa).await.unwrap().tables["works"].len(),
        1
    );
    assert_eq!(server.store.lock().await.files.len(), before_count + 2);
    assert_eq!(
        heads(&drive.list("revision", Some(&library.id)).await.unwrap())
            .unwrap()
            .len(),
        1
    );
    dispose(a, pa).await;
}

#[tokio::test]
async fn corrupt_remote_and_failed_upload_do_not_advance_checkpoint_or_touch_local_records() {
    let server = server().await;
    let drive = Drive::test_server(server.url.clone());
    let library = create_library(&drive, "Failure").await.unwrap();
    let (a, pa) = device().await;
    link_library(&pa, &a, &drive, &library.id).await.unwrap();
    work(&pa, "Keep me").await;
    let before = snapshot::export(&pa).await.unwrap();
    server.store.lock().await.fail_upload = true;
    assert!(sync(&pa, &a, &drive, None).await.is_err());
    assert!(load_link(&a, "main").unwrap().unwrap().revision.is_none());
    server.store.lock().await.fail_upload = false;
    sync(&pa, &a, &drive, None).await.unwrap();
    let checkpoint = serde_json::to_vec(&load_link(&a, "main").unwrap()).unwrap();
    server.store.lock().await.corrupt = true;
    assert!(sync(&pa, &a, &drive, None).await.is_err());
    assert_eq!(snapshot::export(&pa).await.unwrap(), before);
    assert_eq!(
        serde_json::to_vec(&load_link(&a, "main").unwrap()).unwrap(),
        checkpoint
    );
    dispose(a, pa).await;
}

#[tokio::test]
async fn compare_and_swap_rejects_late_edits_and_identical_content_in_another_profile() {
    let (a, pa) = device().await;
    let expected = snapshot::export(&pa).await.unwrap();
    work(&pa, "Late edit").await;
    assert!(
        snapshot::apply_if_unchanged(&pa, &expected, &expected, "main")
            .await
            .is_err()
    );
    let current = snapshot::export(&pa).await.unwrap();
    settings::set_json(
        &pa,
        "_active_profile",
        &serde_json::json!(crate::ids::new_id()),
    )
    .await
    .unwrap();
    assert!(
        snapshot::apply_if_unchanged(&pa, &expected, &current, "main")
            .await
            .is_err()
    );
    assert_eq!(snapshot::export(&pa).await.unwrap(), current);
    dispose(a, pa).await;
}

#[test]
fn sync_planning_never_guesses_when_both_sides_changed() {
    assert!(matches!(plan("a", "a", None, false), Plan::Equal));
    assert!(matches!(plan("a", "b", Some("b"), false), Plan::Push));
    assert!(matches!(plan("a", "b", Some("a"), false), Plan::Pull));
    assert!(matches!(plan("a", "b", None, true), Plan::Pull));
    assert!(matches!(plan("a", "b", None, false), Plan::Conflict));
    assert!(matches!(plan("a", "b", Some("c"), false), Plan::Conflict));
    assert!(validate_name("   ").is_err());
    assert!(validate_uuid("../../escape").is_err());
    assert!(load_link(Path::new("/tmp"), "../main").is_err());
}

#[test]
fn cloud_snapshots_preserve_valuation_tie_order_and_reject_cyclic_histories() {
    let mut data = Snapshot {
        format_version: 4,
        tables: std::collections::BTreeMap::new(),
    };
    let rows: Vec<std::collections::BTreeMap<String, serde_json::Value>> = vec![
        [("id".into(), serde_json::json!("z"))]
            .into_iter()
            .collect(),
        [("id".into(), serde_json::json!("a"))]
            .into_iter()
            .collect(),
    ];
    data.tables.insert("valuation_records".into(), rows.clone());
    assert_eq!(portable(data.clone()).tables["valuation_records"], rows);
    let initial = digest(&data).unwrap();
    data.tables.get_mut("valuation_records").unwrap().reverse();
    assert_ne!(digest(&data).unwrap(), initial);
    let file = |id: &str, parents: Vec<&str>| RemoteFile {
        id: id.into(),
        name: id.into(),
        description: serde_json::to_string(&parents).unwrap(),
        created_time: crate::ids::now_iso(),
        app_properties: Default::default(),
    };
    assert!(heads(&[
        file("a", vec!["b"]),
        file("b", vec!["a"]),
        file("c", vec![])
    ])
    .is_err());
}
