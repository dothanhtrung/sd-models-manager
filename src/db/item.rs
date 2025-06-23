use sqlx::sqlite::SqliteQueryResult;
use sqlx::SqlitePool;
use std::path::PathBuf;

pub async fn mark_all_not_check(pool: &SqlitePool) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query!(r#"UPDATE item SET is_checked=false"#).execute(pool).await
}

pub async fn update_or_insert(pool: &SqlitePool, hash: &str, path: &str, base_id: i64) -> Result<(), sqlx::Error> {
    let pathbuf = PathBuf::from(path);
    let mut parent_id = 0;
    if let Some(parent) = pathbuf.parent() {
        if let Some(parent) = parent.to_str() {
            if let Ok(id) =
                sqlx::query_scalar!(r#"SELECT id FROM item WHERE path = ? and base_id = ?"#, parent, base_id)
                    .fetch_one(pool)
                    .await
            {
                parent_id = id;
            } else {
                parent_id = sqlx::query!(r#"INSERT INTO item (path, base_id) VALUES (?, ?)"#, parent, base_id)
                    .execute(pool)
                    .await?
                    .last_insert_rowid();
            }
        }
    }

    if let Ok(id) = sqlx::query_scalar!(r#"SELECT id FROM item WHERE path = ? AND base_id = ?"#, path, base_id)
        .fetch_one(pool)
        .await
    {
        sqlx::query!(
            r#"UPDATE item SET hash = ?, is_checked=true, parent = ? WHERE id = ?"#,
            hash,
            id,
            parent_id
        )
        .execute(pool)
        .await?;
    } else {
        sqlx::query!(
            r#"INSERT INTO item (hash, path, base_id,  parent) VALUES (?, ?, ?,  ?) "#,
            hash,
            path,
            base_id,
            parent_id
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn clean(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let count = sqlx::query!(r#"DELETE FROM item WHERE is_checked = false"#)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(count)
}
