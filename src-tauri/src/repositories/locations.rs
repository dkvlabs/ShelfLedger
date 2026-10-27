//! Physical location hierarchy (room > bookcase > shelf > box).

use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::ids;
use crate::models::{CreateLocationInput, LocationDto};

pub async fn list_locations(pool: &SqlitePool) -> AppResult<Vec<LocationDto>> {
    let rows: Vec<(String, Option<String>, String, String, i64)> = sqlx::query_as(
        "SELECT id, parent_id, name, type, sort_order FROM locations \
         ORDER BY sort_order, name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, parent_id, name, kind, sort_order)| LocationDto {
            id,
            parent_id,
            name,
            kind,
            sort_order,
        })
        .collect())
}

pub async fn insert_location(
    pool: &SqlitePool,
    input: &CreateLocationInput,
) -> AppResult<LocationDto> {
    let id = ids::new_id();
    let sort_order = input.sort_order.unwrap_or(0);
    let result = sqlx::query(
        "INSERT INTO locations (id, parent_id, name, type, sort_order, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&id)
    .bind(&input.parent_id)
    .bind(input.name.trim())
    .bind(&input.kind)
    .bind(sort_order)
    .bind(ids::now_iso())
    .execute(pool)
    .await
    .map_err(|e| {
        // FK violation -> parent location does not exist.
        if crate::error::is_foreign_key_violation_sqlx(&e) {
            AppError::Validation("parent location does not exist".into())
        } else {
            e.into()
        }
    })?;
    if result.rows_affected() == 0 {
        return Err(AppError::Other("location insert had no effect".into()));
    }
    Ok(LocationDto {
        id,
        parent_id: input.parent_id.clone(),
        name: input.name.trim().to_string(),
        kind: input.kind.clone(),
        sort_order,
    })
}

pub async fn delete_location(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM locations WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            // RESTRICT on parent_id -> cannot delete a location with children.
            if crate::error::is_foreign_key_violation_sqlx(&e) {
                AppError::Conflict(
                    "location still contains sub-locations; delete them first".into(),
                )
            } else {
                e.into()
            }
        })?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn location_crud_and_hierarchy() {
        let pool = crate::db::connect_memory().await.unwrap();
        let room = insert_location(
            &pool,
            &CreateLocationInput {
                name: "Living room".into(),
                kind: "room".into(),
                parent_id: None,
                sort_order: None,
            },
        )
        .await
        .unwrap();
        let bookcase = insert_location(
            &pool,
            &CreateLocationInput {
                name: "IKEA 2x4".into(),
                kind: "bookcase".into(),
                parent_id: Some(room.id.clone()),
                sort_order: Some(2),
            },
        )
        .await
        .unwrap();

        let all = list_locations(&pool).await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|l| l.parent_id == Some(room.id.clone())));

        // Parent with children is protected.
        let err = delete_location(&pool, &room.id).await.unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)));

        assert!(delete_location(&pool, &bookcase.id).await.unwrap());
        assert!(delete_location(&pool, &room.id).await.unwrap());
        assert!(list_locations(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn unknown_parent_is_a_validation_error() {
        let pool = crate::db::connect_memory().await.unwrap();
        let err = insert_location(
            &pool,
            &CreateLocationInput {
                name: "Shelf".into(),
                kind: "shelf".into(),
                parent_id: Some("nope".into()),
                sort_order: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }
}
