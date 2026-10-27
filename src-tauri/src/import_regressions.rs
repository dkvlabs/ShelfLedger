//! Exercise the same file preview/commit commands as the import wizard.
//! The real export stays outside the repository; opt in through an environment variable.
use crate::{commands::data, db, repositories::snapshot, sqlite_db as sqlx};
use serde_json::json;
use std::collections::BTreeMap;

async fn fixture(legacy: bool) -> (std::path::PathBuf, sqlx::SqlitePool) {
    let directory = std::env::temp_dir().join(crate::ids::new_id());
    std::fs::create_dir_all(&directory).unwrap();
    let pool = db::connect(&directory.join("shelfledger.db"))
        .await
        .unwrap();
    if legacy {
        sqlx::query("INSERT INTO works(id,title,created_at,updated_at) VALUES('legacy','Legacy timestamp fixture','x','x')")
            .execute(&pool).await.unwrap();
    }
    (directory, pool)
}

fn selected(count: usize) -> crate::models::ImportCommitInput {
    serde_json::from_value(
        json!({"rowNumbers":(1..=count).collect::<Vec<_>>(),"duplicatePolicy":"skip"}),
    )
    .unwrap()
}

fn sample_csv() -> String {
    let mut csv = csv::Writer::from_writer(Vec::new());
    csv.write_record([
        "Book Id",
        "Title",
        "Author",
        "Additional Authors",
        "ISBN",
        "ISBN13",
        "My Rating",
        "Publisher",
        "Binding",
        "Number of Pages",
        "Year Published",
        "Date Read",
        "Date Added",
        "Bookshelves",
        "Exclusive Shelf",
        "My Review",
        "Owned Copies",
    ])
    .unwrap();
    for row in [
        [
            "1",
            "CSV fixture: quoted, title",
            "Family, Given",
            "Second Writer, Third Writer",
            "=\"054792822X\"",
            "=\"9780547928227\"",
            "5.0",
            "Fixture publisher",
            "Hardcover",
            "320",
            "2020",
            "2024/02/29",
            "2020/01/01",
            "favourites, read",
            "read",
            "First line, with comma\nSecond line — șț你好",
            "0",
        ],
        [
            "2",
            "CSV fixture: ISBN fallback",
            "Other Writer",
            "",
            "=\"080442957X\"",
            "=\"invalid\"",
            "4.00",
            "Another publisher",
            "Paperback",
            "100",
            "1999",
            "",
            "2020/01/01",
            "",
            "read",
            "",
            "0",
        ],
        [
            "3",
            "CSV fixture: manual edition",
            "Third Writer; Another Writer",
            "",
            "=\"\"",
            "=\"\"",
            "0",
            "",
            "Paperback",
            "0",
            "unknown",
            "2024/03/00",
            "2020/01/01",
            "to-read, recommendations",
            "to-read",
            "",
            "0",
        ],
        [
            "4",
            "CSV fixture: invalid optional values",
            "Fourth Writer",
            "",
            "=\"bad\"",
            "=\"123\"",
            "2.5",
            "",
            "Ebook",
            "unknown",
            "",
            "x",
            "2020/01/01",
            "",
            "currently-reading",
            "",
            "0",
        ],
    ] {
        csv.write_record(row).unwrap();
    }
    format!(
        "\u{feff}{}",
        String::from_utf8(csv.into_inner().unwrap()).unwrap()
    )
}

#[tokio::test]
async fn goodreads_file_import_preserves_data_with_and_without_legacy_timestamps() {
    for legacy in [false, true] {
        let (directory, pool) = fixture(legacy).await;
        let path = directory.join("goodreads.csv");
        std::fs::write(&path, sample_csv()).unwrap();
        let path = path.to_str().unwrap();
        let mapping = BTreeMap::new();
        let before = serde_json::to_value(snapshot::export(&pool).await.unwrap()).unwrap();
        let preview = data::preview_import_impl(&pool, path, &mapping)
            .await
            .unwrap();
        assert_eq!(preview["rows"].as_array().unwrap().len(), 4);
        assert_eq!(preview["rows"][0]["rating"], 5);
        assert_eq!(preview["rows"][1]["rating"], 4);
        assert_eq!(preview["rows"][2]["rating"], serde_json::Value::Null);
        assert_eq!(preview["rows"][3]["rating"], serde_json::Value::Null);
        assert_eq!(preview["rows"][1]["isbn13"], "9780804429573");
        assert!(preview["rows"][2]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("no ISBN")));
        assert_eq!(
            before,
            serde_json::to_value(snapshot::export(&pool).await.unwrap()).unwrap(),
            "preview must not write"
        );

        let result = data::commit_import_impl(&pool, path, &mapping, &selected(4))
            .await
            .unwrap();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(
            (
                result.imported_works,
                result.imported_editions,
                result.imported_copies,
                result.skipped
            ),
            (4, 4, 0, 0)
        );
        let metadata: (Option<String>, Option<String>, Option<i64>, Option<String>) = sqlx::query_as("SELECT publisher,binding,page_count,published_at FROM editions WHERE isbn13='9780547928227'").fetch_one(&pool).await.unwrap();
        assert_eq!(
            metadata,
            (
                Some("Fixture publisher".into()),
                Some("Hardcover".into()),
                Some(320),
                Some("2020".into())
            )
        );
        let authors: Vec<String> = sqlx::query_scalar("SELECT a.name FROM authors a JOIN work_authors wa ON wa.author_id=a.id JOIN works w ON wa.work_id=w.id WHERE w.title='CSV fixture: quoted, title' ORDER BY wa.sort_order").fetch_all(&pool).await.unwrap();
        assert_eq!(authors, ["Family, Given", "Second Writer", "Third Writer"]);
        let session: (Option<String>, Option<String>, Option<i64>, Option<String>) =
            sqlx::query_as(
                "SELECT started_at,finished_at,rating,review FROM reading_sessions WHERE rating=5",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            session,
            (
                None,
                Some("2024-02-29".into()),
                Some(5),
                Some("First line, with comma\nSecond line — șț你好".into())
            )
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM reading_sessions WHERE finished_at IS NULL"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            3
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM work_tags")
                .fetch_one(&pool)
                .await
                .unwrap(),
            2
        );
        if legacy {
            let dates: (String, String) =
                sqlx::query_as("SELECT created_at,updated_at FROM works WHERE id='legacy'")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(dates, ("x".into(), "x".into()));
            // Incoming snapshots keep strict validation despite local staging.
            let target = db::connect_memory().await.unwrap();
            assert!(
                snapshot::apply(&target, &snapshot::export(&pool).await.unwrap(), true)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("invalid ISO date: x")
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT count(*) FROM works")
                    .fetch_one(&target)
                    .await
                    .unwrap(),
                0
            );
            target.close().await;
        }
        let again = data::commit_import_impl(&pool, path, &mapping, &selected(4))
            .await
            .unwrap();
        assert!(again.errors.is_empty(), "{:?}", again.errors);
        assert_eq!(
            (
                again.imported_works,
                again.imported_editions,
                again.imported_copies,
                again.skipped
            ),
            (0, 0, 0, 4)
        );
        pool.close().await;
        let reopened = db::connect(&directory.join("shelfledger.db"))
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM works")
                .fetch_one(&reopened)
                .await
                .unwrap(),
            4 + i64::from(legacy)
        );
        reopened.close().await;
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[tokio::test]
async fn json_array_import_ignores_unrelated_legacy_dates_but_validates_new_rows_atomically() {
    let (directory, pool) = fixture(true).await;
    let imported = crate::domain::import::import_json(&pool, r#"[{"title":"Valid new fixture","authorNames":["Writer"],"editions":[{}]},{"title":"Invalid new fixture","authorNames":["Must roll back"],"editions":[{"publishedAt":"x"}]}]"#).await.unwrap();
    assert_eq!(imported.imported_works, 1);
    assert_eq!(imported.errors.len(), 1);
    assert!(imported.errors[0].contains("invalid ISO date: x"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM works")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM authors WHERE name='Must roll back'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    pool.close().await;
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
#[ignore = "local verification: set SHELFLEDGER_TEST_GOODREADS_CSV to a private export"]
async fn private_goodreads_export_imports_every_row_and_repeat_is_idempotent() {
    let path = std::env::var("SHELFLEDGER_TEST_GOODREADS_CSV").expect("set the private CSV path");
    let (directory, pool) = fixture(true).await;
    let preview = data::preview_import_impl(&pool, &path, &BTreeMap::new())
        .await
        .unwrap();
    let rows = preview["rows"].as_array().unwrap();
    assert!(!rows.is_empty());
    let input = selected(rows.len());
    let imported = data::commit_import_impl(&pool, &path, &BTreeMap::new(), &input)
        .await
        .unwrap();
    assert!(imported.errors.is_empty(), "{:?}", imported.errors);
    assert_eq!(imported.imported_works, rows.len() as i64);
    assert_eq!(imported.imported_editions, rows.len() as i64);
    assert_eq!(imported.skipped, 0);
    assert!(imported.rows.iter().all(|row| row.status == "imported"));
    for row in rows {
        let sessions: Vec<(String, Option<String>, Option<i64>)> = sqlx::query_as("SELECT r.status,r.finished_at,r.rating FROM reading_sessions r JOIN works w ON r.work_id=w.id WHERE w.title=?").bind(row["title"].as_str().unwrap()).fetch_all(&pool).await.unwrap();
        let status = match row["shelf"].as_str().unwrap_or("") {
            "read" => "finished",
            "to-read" => "want_to_read",
            "currently-reading" => "reading",
            _ => continue,
        };
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].0, status);
        assert_eq!(
            sessions[0].1.as_deref(),
            if status == "finished" {
                row["dateRead"].as_str()
            } else {
                None
            }
        );
        assert_eq!(sessions[0].2, row["rating"].as_i64());
    }
    let repeated = data::commit_import_impl(&pool, &path, &BTreeMap::new(), &input)
        .await
        .unwrap();
    assert!(repeated.errors.is_empty(), "{:?}", repeated.errors);
    assert_eq!(
        (
            repeated.imported_works,
            repeated.imported_editions,
            repeated.imported_copies,
            repeated.skipped
        ),
        (0, 0, 0, rows.len() as i64)
    );
    let rated: i64 =
        sqlx::query_scalar("SELECT count(*) FROM reading_sessions WHERE rating IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    let dated: i64 =
        sqlx::query_scalar("SELECT count(*) FROM reading_sessions WHERE finished_at IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    println!("Verified {} Goodreads rows: {} ratings, {} finish dates; zero errors; repeat skipped all {} rows", rows.len(),rated,dated,rows.len());
    pool.close().await;
    std::fs::remove_dir_all(directory).unwrap();
}
