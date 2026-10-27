//! Public command regressions for the October audit: test persisted outcomes,
//! rather than merely checking helper calls or return codes.
use crate::{
    db, sqlite_db as sqlx,
    web::{dispatch, WebCtx},
};
use serde_json::{json, Value};

async fn context() -> WebCtx {
    let data_dir = std::env::temp_dir().join(crate::ids::new_id());
    std::fs::create_dir_all(&data_dir).unwrap();
    WebCtx {
        pool: db::connect(&data_dir.join("shelfledger.db")).await.unwrap(),
        data_dir,
        dist: None,
    }
}
async fn call(ctx: &WebCtx, cmd: &str, args: Value) -> Value {
    dispatch(ctx, cmd, &args).await.unwrap()
}
async fn book(ctx: &WebCtx, title: &str) -> Value {
    call(ctx,"add_book",json!({"input":{"title":title,"authorNames":["Last, First"],"description":"A preserved description","copy":{"inventoryCode":"A-17","note":"oldnote","sentimentalScore":4,"acquisition":{"priceMinor":1000,"shippingMinor":200,"taxMinor":50,"currency":"CAD","acquiredAt":"2024-02-03","store":"Local"}}}})).await
}
async fn cleanup(ctx: WebCtx) {
    ctx.pool.close().await;
    crate::file_cleanup::remove_dir_all(&ctx.data_dir)
        .await
        .unwrap();
}

#[tokio::test]
async fn restore_with_live_wal_keeps_post_restore_writes_and_safety_copy() {
    let ctx = context().await;
    book(&ctx, "A").await;
    let archive = call(&ctx, "create_backup", json!({})).await["path"]
        .as_str()
        .unwrap()
        .to_string();
    book(&ctx, "B").await;
    let restored = call(
        &ctx,
        "restore_backup",
        json!({"archivePath":archive,"mode":"replace"}),
    )
    .await;
    assert_eq!(restored["requiresRestart"], false);
    book(&ctx, "C").await;
    ctx.pool.close().await;
    let pool = db::connect(&ctx.data_dir.join("shelfledger.db"))
        .await
        .unwrap();
    let titles: Vec<(String,)> = sqlx::query_as("SELECT title FROM works ORDER BY title")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(titles, vec![("A".into(),), ("C".into(),)]);
    let safety = std::fs::read_dir(&ctx.data_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.to_string_lossy().contains("pre-restore-"))
        .unwrap();
    crate::domain::backup::restore_archive(
        &pool,
        &ctx.data_dir.join("shelfledger.db"),
        &safety,
        "replace",
    )
    .await
    .unwrap();
    let titles: Vec<(String,)> = sqlx::query_as("SELECT title FROM works ORDER BY title")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(titles, vec![("A".into(),), ("B".into(),)]);
    pool.close().await;
    crate::file_cleanup::remove_dir_all(&ctx.data_dir)
        .await
        .unwrap();
}

#[tokio::test]
async fn complete_snapshot_preserves_history_and_rejects_duplicates_atomically() {
    let ctx = context().await;
    let b = book(&ctx, "History").await;
    let work = b["workId"].clone();
    let copy = b["copyId"].clone();
    call(
        &ctx,
        "set_work_tags",
        json!({"id":work,"tags":["Favourites","Essays"]}),
    )
    .await;
    let s = call(
        &ctx,
        "create_reading_session",
        json!({"input":{"workId":work,"status":"reading","startedAt":"2024-01-01"}}),
    )
    .await;
    call(
        &ctx,
        "add_reading_progress",
        json!({"sessionId":s["id"],"input":{"page":12,"occurredAt":"2024-01-02"}}),
    )
    .await;
    call(&ctx,"update_reading_session",json!({"id":s["id"],"input":{"status":"finished","finishedAt":"2024-01-03","rating":5,"review":"A review"}})).await;
    let contact = call(
        &ctx,
        "create_contact",
        json!({"input":{"displayName":"Reader"}}),
    )
    .await;
    let loan = call(
        &ctx,
        "lend_copy",
        json!({"input":{"copyId":copy,"contactId":contact["id"],"lentAt":"2024-03-01"}}),
    )
    .await;
    call(
        &ctx,
        "return_loan",
        json!({"loanId":loan["id"],"input":{"returnedAt":"2024-03-02"}}),
    )
    .await;
    call(&ctx,"add_valuation",json!({"input":{"copyId":copy,"valueMinor":900,"currency":"CAD","sourceType":"manual","confidence":"low","observedAt":"2024-04-01"}})).await;
    call(
        &ctx,
        "create_listing",
        json!({"input":{"copyId":copy,"askingPriceMinor":900,"currency":"CAD"}}),
    )
    .await;
    call(&ctx,"record_disposal",json!({"input":{"copyId":copy,"disposition":"sold","salePriceMinor":900,"feeMinor":20,"currency":"CAD","date":"2024-04-02"}})).await;
    call(&ctx, "archive_copy", json!({"id":copy})).await;
    let snapshot = crate::repositories::snapshot::export(&ctx.pool)
        .await
        .unwrap();
    let target = db::connect_memory().await.unwrap();
    crate::repositories::snapshot::apply(&target, &snapshot, true)
        .await
        .unwrap();
    let original = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        original,
        serde_json::to_value(
            crate::repositories::snapshot::export(&target)
                .await
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(original["tables"]["authors"][0]["name"], "Last, First");
    assert_eq!(original["tables"]["disposals"].as_array().unwrap().len(), 1);
    let mut invalid = snapshot;
    let repeated = invalid.tables["works"][0].clone();
    invalid.tables.get_mut("works").unwrap().push(repeated);
    assert!(
        crate::repositories::snapshot::apply(&target, &invalid, true)
            .await
            .is_err()
    );
    assert_eq!(
        original,
        serde_json::to_value(
            crate::repositories::snapshot::export(&target)
                .await
                .unwrap()
        )
        .unwrap()
    );
    cleanup(ctx).await;
}

#[tokio::test]
async fn import_failure_does_not_leave_work_edition_or_first_copy() {
    let ctx = context().await;
    let text = r#"[{"title":"Atomic","authorNames":["Author"],"editions":[{"copies":[{"conditionGrade":"good"},{"conditionGrade":"invalid"}]}]}]"#;
    let result = crate::domain::import::import_json(&ctx.pool, text)
        .await
        .unwrap();
    assert_eq!(result.imported_works, 0);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.rows[0].status, "failed");
    for table in ["works", "editions", "copies", "authors"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&ctx.pool)
                .await
                .unwrap(),
            0
        );
    }
    cleanup(ctx).await;
}

#[tokio::test]
async fn goodreads_exclusive_shelf_reviews_tags_unknown_dates_and_row_log() {
    let ctx = context().await;
    let csv="Title,Author,ISBN13,ISBN,Exclusive Shelf,Bookshelves,My Rating,My Review,Date Read\nReading history,Writer,invalid,080442957X,read,favourites,5,A review,\nUnicode date,Other,,,,read,,,🙂🙂🙂🙂\n";
    let input =
        serde_json::from_value(json!({"rowNumbers":[1,2],"duplicatePolicy":"skip"})).unwrap();
    let result = crate::domain::import::commit_csv_text(&ctx.pool, csv, &input)
        .await
        .unwrap();
    assert_eq!(result.imported_works, 2, "{:?}", result.errors);
    let sessions = call(&ctx, "list_reading_sessions", json!({})).await;
    let session = sessions
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["workTitle"] == "Reading history")
        .unwrap();
    assert_eq!(session["finishedAt"], Value::Null);
    assert_eq!(session["startedAt"], Value::Null);
    assert_eq!(session["review"], "A review");
    assert_eq!(session["rating"], 5);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT isbn13 FROM editions WHERE isbn13 IS NOT NULL")
            .fetch_one(&ctx.pool)
            .await
            .unwrap(),
        "9780804429573"
    );
    let repeated = crate::domain::import::commit_csv_text(&ctx.pool, csv, &input)
        .await
        .unwrap();
    assert_eq!(repeated.skipped, 2);
    assert!(repeated.rows.iter().all(|r| r.status == "skipped"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM work_tags")
            .fetch_one(&ctx.pool)
            .await
            .unwrap(),
        1
    );
    cleanup(ctx).await;
}

#[tokio::test]
async fn patch_and_every_copy_lifecycle_mutation_keep_search_current() {
    let ctx = context().await;
    let b = book(&ctx, "Search").await;
    let id = b["copyId"].clone();
    let search = |q: &str| json!({"query":q});
    assert_eq!(
        call(&ctx, "search_works", search("oldnote"))
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let patched = call(
        &ctx,
        "update_copy",
        json!({"id":id,"input":{"note":"newnote"}}),
    )
    .await;
    assert_eq!(patched["conditionGrade"], "good");
    assert_eq!(patched["sentimentalScore"], 4);
    assert_eq!(
        call(&ctx, "search_works", search("oldnote")).await,
        json!([])
    );
    assert_eq!(
        call(&ctx, "search_works", search("newnote"))
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    call(&ctx, "archive_copy", json!({"id":id})).await;
    assert_eq!(
        call(&ctx, "search_works", search("newnote")).await,
        json!([])
    );
    assert_eq!(
        call(&ctx, "search_works", search("Search")).await[0]["copyCount"],
        0
    );
    call(&ctx, "unarchive_copy", json!({"id":id})).await;
    let clear = call(
        &ctx,
        "update_copy",
        json!({"id":id,"input":{"note":null,"sentimentalScore":null}}),
    )
    .await;
    assert!(clear["note"].is_null());
    assert!(clear["sentimentalScore"].is_null());
    let location = call(
        &ctx,
        "create_location",
        json!({"input":{"name":"Moonroom","type":"shelf"}}),
    )
    .await;
    call(
        &ctx,
        "update_copy",
        json!({"id":id,"input":{"locationId":location["id"]}}),
    )
    .await;
    assert_eq!(
        call(&ctx, "search_works", search("Moonroom"))
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    call(&ctx, "delete_location", json!({"id":location["id"]})).await;
    assert_eq!(
        call(&ctx, "search_works", search("Moonroom")).await,
        json!([])
    );
    cleanup(ctx).await;
}

#[tokio::test]
async fn same_date_values_select_one_and_disposal_cannot_resurrect_copy() {
    let ctx = context().await;
    let b = book(&ctx, "Finance").await;
    let copy = b["copyId"].clone();
    for amount in [100, 200] {
        call(&ctx,"add_valuation",json!({"input":{"copyId":copy,"valueMinor":amount,"currency":"CAD","sourceType":"manual","confidence":"low","observedAt":"2024-01-01"}})).await;
    }
    assert_eq!(
        call(&ctx, "get_finance_report", json!({})).await["estimatedValue"],
        json!([{"currency":"CAD","totalMinor":200}])
    );
    assert_eq!(
        call(&ctx, "get_sell_queue", json!({})).await[0]["latestValuation"]["valueMinor"],
        200
    );
    let listing = call(
        &ctx,
        "create_listing",
        json!({"input":{"copyId":copy,"askingPriceMinor":200,"currency":"CAD"}}),
    )
    .await;
    assert!(dispatch(
        &ctx,
        "create_listing",
        &json!({"input":{"copyId":copy,"askingPriceMinor":300,"currency":"CAD"}})
    )
    .await
    .is_err());
    call(
        &ctx,
        "record_disposal",
        json!({"input":{"copyId":copy,"disposition":"donated"}}),
    )
    .await;
    assert!(dispatch(
        &ctx,
        "withdraw_listing",
        &json!({"listingId":listing["id"]})
    )
    .await
    .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT ownership_status FROM copies")
            .fetch_one(&ctx.pool)
            .await
            .unwrap(),
        "donated"
    );
    cleanup(ctx).await;
}

#[tokio::test]
async fn invalid_domain_inputs_never_persist_and_cross_work_reading_is_rejected() {
    let ctx = context().await;
    for patch in [
        json!({"pageCount":-1}),
        json!({"publishedAt":"2024-02-30"}),
        json!({"isbn10":"0306406152","isbn13":"9780547928227"}),
        json!({"copy":{"acquisition":{"shippingMinor":10}}}),
        json!({"copy":{"acquiredAt":"yesterday"}}),
    ] {
        let mut input = json!({"title":"Invalid","authorNames":[]});
        input
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(
            dispatch(&ctx, "add_book", &json!({"input":input}))
                .await
                .is_err(),
            "{input}"
        );
    }
    assert_eq!(
        call(&ctx, "get_library_stats", json!({})).await["workCount"],
        0
    );
    let a = book(&ctx, "One").await;
    let b = book(&ctx, "Two").await;
    assert!(dispatch(
        &ctx,
        "create_reading_session",
        &json!({"input":{"workId":a["workId"],"editionId":b["editionId"],"status":"reading"}})
    )
    .await
    .is_err());
    cleanup(ctx).await;
}

#[tokio::test]
async fn dashboard_aggregates_years_once_per_currency() {
    let ctx = context().await;
    book(&ctx, "2024 CAD").await;
    let b = book(&ctx, "2025 CAD").await;
    sqlx::query("UPDATE acquisitions SET acquired_at='2025-01-01' WHERE copy_id=?")
        .bind(b["copyId"].as_str().unwrap())
        .execute(&ctx.pool)
        .await
        .unwrap();
    let result = call(&ctx, "get_dashboard", json!({"localToday":"2026-10-27"})).await;
    assert_eq!(
        result["spendingByCurrency"],
        json!([{"currency":"CAD","totalMinor":2500}])
    );
    cleanup(ctx).await;
}

#[tokio::test]
async fn mapped_csv_import_commits_only_confirmed_rows_and_rejects_ambiguous_mappings() {
    let ctx = context().await;
    let path = ctx.data_dir.join("mapped.csv");
    std::fs::write(&path,"Book name,Writer,Shelf,Stars,Notes\nFirst,Tester,read,4,Keep this review\nSecond,Tester,to-read,0,\n").unwrap();
    let mapping = json!({"Title":"Book name","Author":"Writer","Exclusive Shelf":"Shelf","My Rating":"Stars","My Review":"Notes"});
    let preview = call(
        &ctx,
        "preview_import",
        json!({"path":path,"mapping":mapping}),
    )
    .await;
    assert_eq!(preview["rows"][0]["title"], "First");
    assert_eq!(
        call(&ctx, "get_library_stats", json!({})).await["workCount"],
        0
    );
    assert!(dispatch(
        &ctx,
        "preview_import",
        &json!({"path":path,"mapping":{"Title":"Book name","Author":"Book name"}})
    )
    .await
    .is_err());
    let result = call(
        &ctx,
        "commit_import",
        json!({"path":path,"mapping":mapping,"input":{"rowNumbers":[1],"duplicatePolicy":"skip"}}),
    )
    .await;
    assert_eq!(result["importedWorks"], 1);
    assert_eq!(result["rows"][0]["status"], "imported");
    let review: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT review,finished_at FROM reading_sessions")
            .fetch_one(&ctx.pool)
            .await
            .unwrap();
    assert_eq!(review, (Some("Keep this review".into()), None));
    assert_eq!(
        call(&ctx, "get_library_stats", json!({})).await["workCount"],
        1
    );
    cleanup(ctx).await;
}

#[tokio::test]
async fn catalog_editors_preserve_identity_history_and_reject_conflicting_editions() {
    let ctx = context().await;
    let b = book(&ctx, "Editable").await;
    call(&ctx,"update_edition",json!({"id":b["editionId"],"input":{"workId":b["workId"],"publisher":"Corrected","pageCount":123,"publishedAt":"2024"}})).await;
    call(&ctx,"update_copy_details",json!({"id":b["copyId"],"input":{"editionId":b["editionId"],"conditionGrade":"poor","inventoryCode":"EDIT-2","note":"editsearchtoken","acquisition":{"priceMinor":1234,"currency":"CHF","shippingMinor":100}}})).await;
    let detail = call(&ctx, "get_work", json!({"id":b["workId"]})).await;
    let copy = &detail["editions"][0]["copies"];
    assert_eq!(copy[0]["ownershipStatus"], "owned");
    assert_eq!(copy[0]["acquisition"]["priceMinor"], 1234);
    assert_eq!(copy[0]["acquisition"]["currency"], "CHF");
    assert_eq!(
        crate::repositories::works::search_fts_ids(&ctx.pool, "editsearchtoken")
            .await
            .unwrap(),
        vec![b["workId"].as_str().unwrap()]
    );
    let other = book(&ctx, "Other").await;
    assert!(dispatch(
        &ctx,
        "update_edition",
        &json!({"id":b["editionId"],"input":{"workId":other["workId"]}})
    )
    .await
    .is_err());
    call(&ctx, "archive_copy", json!({"id":b["copyId"]})).await;
    assert!(dispatch(
        &ctx,
        "update_copy_details",
        &json!({"id":b["copyId"],"input":{"editionId":b["editionId"],"conditionGrade":"good"}})
    )
    .await
    .is_err());
    call(&ctx, "unarchive_copy", json!({"id":b["copyId"]})).await;
    assert_eq!(
        call(&ctx, "get_library_stats", json!({})).await["copyCount"],
        2
    );
    cleanup(ctx).await;
}

#[tokio::test]
async fn hundred_book_offline_soak_survives_reopen_and_remains_searchable() {
    let ctx = context().await;
    for number in 0..100 {
        book(&ctx, &format!("Soak volume {number:03}")).await;
    }
    assert_eq!(
        call(&ctx, "get_library_stats", json!({})).await["workCount"],
        100
    );
    ctx.pool.close().await;
    let pool = db::connect(&ctx.data_dir.join("shelfledger.db"))
        .await
        .unwrap();
    assert_eq!(
        crate::repositories::works::search_fts_ids(&pool, "Soak")
            .await
            .unwrap()
            .len(),
        100
    );
    assert_eq!(
        crate::repositories::works::search_fts_ids(&pool, "099")
            .await
            .unwrap()
            .len(),
        1
    );
    pool.close().await;
    crate::file_cleanup::remove_dir_all(&ctx.data_dir)
        .await
        .unwrap();
}
