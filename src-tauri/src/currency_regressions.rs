//! Currency-scale upgrades preserve what users entered in old versions.
use crate::{db, sqlite_db as sqlx};
use sqlx::SqlitePool;

async fn old_library() -> SqlitePool {
    let pool = db::connect_memory().await.unwrap();
    // 0005 changes data only; removing its stamp recreates the v4 schema.
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version=5")
        .execute(&pool)
        .await
        .unwrap();
    let statements = [
        "INSERT INTO works(id,title,created_at,updated_at) VALUES('w','History','2026-01-01','2026-01-01')",
        "INSERT INTO editions(id,work_id,created_at) VALUES('e','w','2026-01-01')",
        "INSERT INTO copies(id,edition_id,created_at,updated_at) VALUES('c','e','2026-01-01','2026-01-01')",
        "INSERT INTO acquisitions(id,copy_id,price_minor,shipping_minor,tax_minor,currency,created_at) VALUES('a','c',12,2,1,'PKR','2026-01-01')",
        "INSERT INTO valuation_records(id,copy_id,value_minor,currency,source_type,observed_at,confidence,created_at) VALUES('v','c',20,'IQD','manual','2026-01-01','low','2026-01-01')",
        "INSERT INTO sale_listings(id,copy_id,asking_price_minor,currency,status,listed_at,created_at,updated_at) VALUES('s','c',30,'AFN','withdrawn','2026-01-01','2026-01-01','2026-01-01')",
        "INSERT INTO disposals(id,copy_id,disposition,date,sale_price_minor,fee_minor,shipping_minor,currency,created_at) VALUES('d','c','sold','2026-01-01',40,3,2,'IDR','2026-01-01')",
        "INSERT INTO wishlist(id,work_id,maximum_price_minor,currency,created_at) VALUES('q','w',50,'HUF','2026-01-01')",
    ];
    for statement in statements {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    pool
}
async fn amounts(pool: &SqlitePool) -> Vec<i64> {
    sqlx::query_scalar("SELECT price_minor FROM acquisitions UNION ALL SELECT shipping_minor FROM acquisitions UNION ALL SELECT tax_minor FROM acquisitions UNION ALL SELECT value_minor FROM valuation_records UNION ALL SELECT asking_price_minor FROM sale_listings UNION ALL SELECT sale_price_minor FROM disposals UNION ALL SELECT fee_minor FROM disposals UNION ALL SELECT shipping_minor FROM disposals UNION ALL SELECT maximum_price_minor FROM wishlist").fetch_all(pool).await.unwrap()
}
#[tokio::test]
async fn migration_and_old_json_preserve_all_money_fields_exactly_once() {
    let old = old_library().await;
    let mut snapshot = crate::repositories::snapshot::export(&old).await.unwrap();
    snapshot.format_version = 1;
    db::MIGRATOR.run(&old).await.unwrap();
    let expected = vec![1200, 200, 100, 20000, 3000, 4000, 300, 200, 5000];
    assert_eq!(amounts(&old).await, expected);
    db::MIGRATOR.run(&old).await.unwrap();
    assert_eq!(amounts(&old).await, expected);
    let imported = db::connect_memory().await.unwrap();
    crate::repositories::snapshot::apply(&imported, &snapshot, true)
        .await
        .unwrap();
    assert_eq!(amounts(&imported).await, expected);
    let current = crate::repositories::snapshot::export(&imported)
        .await
        .unwrap();
    assert_eq!(current.format_version, 3);
    crate::repositories::snapshot::apply(&imported, &current, true)
        .await
        .unwrap();
    assert_eq!(amounts(&imported).await, expected);
}
#[tokio::test]
async fn unsafe_upgrade_rolls_back_migration_and_json_import() {
    let old = old_library().await;
    sqlx::query("UPDATE wishlist SET maximum_price_minor=9007199254740991")
        .execute(&old)
        .await
        .unwrap();
    let before = amounts(&old).await;
    let mut snapshot = crate::repositories::snapshot::export(&old).await.unwrap();
    snapshot.format_version = 1;
    assert!(db::MIGRATOR.run(&old).await.is_err());
    assert_eq!(amounts(&old).await, before);
    let target = db::connect_memory().await.unwrap();
    assert!(
        crate::repositories::snapshot::apply(&target, &snapshot, true)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM works")
        .fetch_one(&target)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
