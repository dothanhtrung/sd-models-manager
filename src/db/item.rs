use sqlx::sqlite::SqliteQueryResult;
use sqlx::SqlitePool;
use std::path::PathBuf;

pub struct Item {
    pub id: i64,
    pub path: String,
    pub base_id: i64,
    pub hash: Option<String>,
    pub is_checked: i64,
    pub parent: Option<i64>,
}

pub async fn mark_all_not_check(pool: &SqlitePool) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query!(r#"UPDATE item SET is_checked = false"#)
        .execute(pool)
        .await
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

pub async fn get(pool: &SqlitePool, parent: i64, limit: i64, offset: i64) -> Result<Vec<Item>, sqlx::Error> {
    let items = sqlx::query_as!(
        Item,
        r#"SELECT * FROM item WHERE parent = ? AND is_checked = true ORDER BY id LIMIT ? OFFSET ?"#,
        parent,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    Ok(items)
}

pub async fn get_root(pool: &SqlitePool, limit: i64, offset: i64) -> Result<Vec<Item>, sqlx::Error> {
    let items = sqlx::query_as!(
        Item,
        r#"SELECT * FROM item WHERE path = "" AND is_checked = true ORDER BY id LIMIT ? OFFSET ?"#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;
    Ok(items)
}
