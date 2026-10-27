use crate::{
    db,
    domain::market,
    repositories::snapshot,
    sqlite_db as sqlx,
    web::{dispatch, WebCtx},
};
use serde_json::{json, Value};

async fn context() -> (WebCtx, Value) {
    let data_dir = std::env::temp_dir().join(crate::ids::new_id());
    std::fs::create_dir_all(&data_dir).unwrap();
    let ctx = WebCtx {
        pool: db::connect(&data_dir.join("library.db")).await.unwrap(),
        data_dir,
        dist: None,
    };
    let book = call(
        &ctx,
        "add_book",
        json!({"input":{"title":"QA fixture, not market data","authorNames":[],"copy":{}}}),
    )
    .await;
    (ctx, book)
}
async fn cleanup(ctx: WebCtx) {
    ctx.pool.close().await;
    std::fs::remove_dir_all(ctx.data_dir).unwrap();
}
async fn call(ctx: &WebCtx, cmd: &str, args: Value) -> Value {
    dispatch(ctx, cmd, &args).await.unwrap()
}
fn input(book: &Value, slug: &str, price: i64, currency: &str) -> Value {
    json!({"input":{"editionId":book["editionId"],"listingUrl":format!("https://www.olx.ro/d/oferta/qa-{slug}-IDtest.html"),"askingMinor":price,"currency":currency,"observedAt":"2026-10-27","conditionNote":"QA fixture — not market data","editionChecked":true}})
}
async fn report(ctx: &WebCtx, book: &Value) -> Value {
    call(
        ctx,
        "list_market_comparisons",
        json!({"editionId":book["editionId"]}),
    )
    .await
}
fn estimate(book: &Value, currency: &str) -> Value {
    json!({"input":{"editionId":book["editionId"],"copyId":book["copyId"],"currency":currency,"confidence":"low"}})
}

#[test]
fn only_public_olx_romania_https_links_can_be_opened() {
    let good = "https://www.olx.ro/d/oferta/book-IDtest.html";
    assert_eq!(
        market::olx_url(
            "https://olx.ro/d/oferta/book-IDtest.html?tracking=1#x",
            true
        )
        .unwrap(),
        good
    );
    for url in [
        "javascript:alert(1)",
        "http://www.olx.ro/d/oferta/book-IDtest.html",
        "https://olx.ro.evil.test/d/oferta/book-IDtest.html",
        "https://user@olx.ro/d/oferta/book-IDtest.html",
        "https://olx.ro:444/d/oferta/book-IDtest.html",
        "https://olx.ro/account/",
        "https://other.test/d/oferta/book-IDtest.html",
    ] {
        assert!(market::olx_url(url, false).is_err(), "{url}");
    }
    let search = "https://www.olx.ro/hobby-sport-turism/carti-muzica-filme/carti/q-9780306406157/";
    assert!(market::olx_url(search, false).is_ok());
    assert!(market::olx_url(search, true).is_err());
}
#[tokio::test]
async fn validates_before_writes_and_foreign_keys_remain_enforced() {
    let (ctx, book) = context().await;
    for (key, value) in [
        ("listingUrl", json!("https://example.com/book")),
        ("askingMinor", json!(-1)),
        ("askingMinor", json!(9_007_199_254_740_992_i64)),
        ("currency", json!("INVALID")),
        ("editionChecked", json!(false)),
        ("observedAt", json!("2026-02-30")),
        ("observedAt", json!("")),
        ("conditionNote", json!("x".repeat(2001))),
        ("editionId", json!("missing")),
    ] {
        let mut args = input(&book, "a", 1200, "RON");
        args["input"][key] = value;
        assert!(
            dispatch(&ctx, "save_market_comparison", &args)
                .await
                .is_err(),
            "{key}"
        );
    }
    assert!(report(&ctx, &book).await["comparables"]
        .as_array()
        .unwrap()
        .is_empty());
    let bad=sqlx::query("INSERT INTO market_comparables VALUES ('bad','missing','https://www.olx.ro/d/oferta/book-IDtest.html',0,'RON',NULL,'2026-10-27',1,'2026-10-27','2026-10-27')").execute(&ctx.pool).await;
    assert!(bad.is_err());
    cleanup(ctx).await;
}
#[tokio::test]
async fn urls_deduplicate_updates_and_exclusions_survive_reopening() {
    let (ctx, book) = context().await;
    call(
        &ctx,
        "save_market_comparison",
        input(&book, "a", 1000, "ron"),
    )
    .await;
    let first = report(&ctx, &book).await["comparables"][0].clone();
    let mut update = input(&book, "a", 1250, "RON");
    update["input"]["listingUrl"] = json!(format!(
        "{}?tracking=ignored#fragment",
        first["listingUrl"].as_str().unwrap()
    ));
    call(&ctx, "save_market_comparison", update).await;
    let updated = report(&ctx, &book).await;
    assert_eq!(updated["comparables"].as_array().unwrap().len(), 1);
    assert_eq!(updated["comparables"][0]["id"], first["id"]);
    assert_eq!(updated["summaries"][0]["medianMinor"], 1250);
    call(
        &ctx,
        "set_market_comparison_included",
        json!({"id":first["id"],"included":false}),
    )
    .await;
    ctx.pool.close().await;
    let reopened = WebCtx {
        pool: db::connect(&ctx.data_dir.join("library.db")).await.unwrap(),
        data_dir: ctx.data_dir,
        dist: None,
    };
    let persisted = report(&reopened, &book).await;
    assert_eq!(persisted["comparables"][0]["included"], false);
    assert!(persisted["summaries"].as_array().unwrap().is_empty());
    assert!(
        dispatch(&reopened, "save_market_estimate", &estimate(&book, "RON"))
            .await
            .is_err()
    );
    cleanup(reopened).await;
}
#[tokio::test]
async fn summaries_partition_currencies_and_round_only_half_minor_units() {
    let (ctx, book) = context().await;
    for (slug, price, currency) in [
        ("a", 1000, "RON"),
        ("b", 2001, "RON"),
        ("c", 99, "JPY"),
        ("d", 9_007_199_254_740_991, "USD"),
        ("e", 9_007_199_254_740_989, "USD"),
    ] {
        call(
            &ctx,
            "save_market_comparison",
            input(&book, slug, price, currency),
        )
        .await;
    }
    let r = report(&ctx, &book).await;
    assert_eq!(
        r["summaries"],
        json!([
            {"currency":"JPY","count":1,"minMinor":99,"maxMinor":99,"medianMinor":99,"medianRounded":false},
            {"currency":"RON","count":2,"minMinor":1000,"maxMinor":2001,"medianMinor":1500,"medianRounded":true},
            {"currency":"USD","count":2,"minMinor":9_007_199_254_740_989_i64,"maxMinor":9_007_199_254_740_991_i64,"medianMinor":9_007_199_254_740_990_i64,"medianRounded":false}
        ])
    );
    let id = &r["comparables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["askingMinor"] == 1000)
        .unwrap()["id"];
    call(
        &ctx,
        "set_market_comparison_included",
        json!({"id":id,"included":false}),
    )
    .await;
    assert_eq!(
        report(&ctx, &book).await["summaries"][1]["medianMinor"],
        2001
    );
    cleanup(ctx).await;
}
#[tokio::test]
async fn estimates_are_explicit_copy_scoped_and_preserve_immutable_evidence() {
    let (ctx, book) = context().await;
    assert!(
        dispatch(&ctx, "save_market_estimate", &estimate(&book, "RON"))
            .await
            .is_err()
    );
    call(
        &ctx,
        "save_market_comparison",
        input(&book, "a", 1000, "RON"),
    )
    .await;
    call(
        &ctx,
        "save_market_comparison",
        input(&book, "b", 2000, "RON"),
    )
    .await;
    assert!(
        call(&ctx, "list_valuations", json!({"copyId":book["copyId"]}))
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        dispatch(&ctx, "save_market_estimate", &estimate(&book, "EUR"))
            .await
            .is_err()
    );
    let other = call(
        &ctx,
        "add_book",
        json!({"input":{"title":"Other edition","copy":{}}}),
    )
    .await;
    let mut bad = estimate(&book, "RON");
    bad["input"]["copyId"] = other["copyId"].clone();
    assert!(dispatch(&ctx, "save_market_estimate", &bad).await.is_err());
    let saved = call(&ctx, "save_market_estimate", estimate(&book, "RON")).await;
    assert_eq!(saved["valueMinor"], 1500);
    assert_eq!(saved["confidence"], "low");
    assert_eq!(saved["sourceType"], "comparable");
    let evidence = saved["sourceNote"].as_str().unwrap();
    for text in [
        "not completed sales",
        "qa-a-IDtest.html",
        "qa-b-IDtest.html",
        "2026-10-27",
        "1000 RON minor units",
        "QA fixture",
    ] {
        assert!(evidence.contains(text), "{text}");
    }
    call(
        &ctx,
        "save_market_comparison",
        input(&book, "a", 5000, "RON"),
    )
    .await;
    let values = call(&ctx, "list_valuations", json!({"copyId":book["copyId"]})).await;
    assert_eq!(values[0], saved);
    call(&ctx, "archive_copy", json!({"id":book["copyId"]})).await;
    assert!(
        dispatch(&ctx, "save_market_estimate", &estimate(&book, "RON"))
            .await
            .is_err()
    );
    cleanup(ctx).await;
}
#[tokio::test]
async fn snapshot_three_roundtrips_evidence_and_imports_old_complete_exports() {
    let (ctx, book) = context().await;
    call(
        &ctx,
        "save_market_comparison",
        input(&book, "a", 1000, "RON"),
    )
    .await;
    call(&ctx, "save_market_estimate", estimate(&book, "RON")).await;
    let current = snapshot::export(&ctx.pool).await.unwrap();
    assert_eq!(current.format_version, 3);
    let target = db::connect_memory().await.unwrap();
    snapshot::apply(&target, &current, true).await.unwrap();
    assert_eq!(
        serde_json::to_value(snapshot::export(&target).await.unwrap()).unwrap(),
        serde_json::to_value(&current).unwrap()
    );
    for version in [1, 2] {
        let mut legacy = snapshot::export(&ctx.pool).await.unwrap();
        legacy.format_version = version;
        legacy.tables.remove("market_comparables");
        snapshot::apply(&target, &legacy, true).await.unwrap();
        assert!(market::report(&target, book["editionId"].as_str().unwrap())
            .await
            .unwrap()
            .comparables
            .is_empty());
    }
    let mut bad = snapshot::export(&ctx.pool).await.unwrap();
    bad.tables.get_mut("market_comparables").unwrap()[0]
        .insert("listing_url".into(), json!("https://evil.test/"));
    assert!(snapshot::apply(&target, &bad, true).await.is_err());
    assert!(market::report(&target, book["editionId"].as_str().unwrap())
        .await
        .unwrap()
        .comparables
        .is_empty());
    target.close().await;
    cleanup(ctx).await;
}
