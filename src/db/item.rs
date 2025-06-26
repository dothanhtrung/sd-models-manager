use sqlx::sqlite::SqliteQueryResult;
use sqlx::SqlitePool;
use std::path::PathBuf;

pub struct Item {
    pub id: i64,
    pub name: Option<String>,
    pub path: String,
}

pub async fn mark_obsolete_all(pool: &SqlitePool) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query!(r#"UPDATE item SET is_checked = false WHERE is_checked = true AND path != ''"#)
        .execute(pool)
        .await
}

/// Return (path, label)
pub async fn mark_obsolete(pool: &SqlitePool, id: i64) -> Result<(String, String), sqlx::Error> {
    sqlx::query!(r#"UPDATE item SET is_checked = false WHERE id = ?"#, id)
        .execute(pool)
        .await?;

    struct Temp {
        path: String,
        label: String,
    };
    let ret = sqlx::query_as!(
        Temp,
        r#"SELECT item.path, base.label FROM base INNER JOIN item ON base.id = item.base_id WHERE item.id = ?"#,
        id
    )
    .fetch_one(pool)
    .await?;

    Ok((ret.path, ret.label))
}

pub async fn insert_or_update(
    pool: &SqlitePool,
    name: Option<&str>,
    path: &str,
    base_id: i64,
) -> Result<(), sqlx::Error> {
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
                parent_id = sqlx::query!(r#"INSERT OR IGNORE INTO item ( path, base_id) VALUES (?, ?)"#, parent, base_id)
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
            r#"UPDATE item SET is_checked=true, parent = ? WHERE id = ?"#,
            parent_id,
            id,
        )
        .execute(pool)
        .await?;
    } else if parent_id != 0 {
        sqlx::query!(
            r#"INSERT OR IGNORE INTO item (name, path, base_id, parent) VALUES (?, ?, ?, ?) "#,
            name,
            path,
            base_id,
            parent_id
        )
        .execute(pool)
        .await?;
    } else {
        sqlx::query!(
            r#"INSERT OR IGNORE INTO item (name, path, base_id) VALUES (?, ?,  ?) "#,
            name,
            path,
            base_id,
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

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<(Item, String), sqlx::Error> {
    let item = sqlx::query_as!(Item, "SELECT id, name, path FROM item WHERE id = ?", id)
        .fetch_one(pool)
        .await?;
    let label = sqlx::query_scalar!(
        "SELECT label FROM base INNER JOIN item ON base.id = item.base_id WHERE item.id = ?",
        id
    )
    .fetch_one(pool)
    .await?;

    Ok((item, label))
}

pub async fn get(pool: &SqlitePool, parent: i64, limit: i64, offset: i64) -> Result<(Vec<Item>, i64), sqlx::Error> {
    let items = sqlx::query_as!(
        Item,
        r#"SELECT id, name, path FROM item WHERE parent = ? AND is_checked = true ORDER BY id DESC LIMIT ? OFFSET ?"#,
        parent,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar!(
        "SELECT count(id) FROM item WHERE parent = ? AND is_checked = true",
        parent
    )
    .fetch_one(pool)
    .await?;

    Ok((items, total))
}

pub async fn get_root(pool: &SqlitePool, limit: i64, offset: i64) -> Result<(Vec<Item>, i64), sqlx::Error> {
    let items = sqlx::query_as!(
        Item,
        r#"SELECT id, name, path FROM item WHERE path = '' AND is_checked = true ORDER BY id LIMIT ? OFFSET ?"#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar!(r#"SELECT count(id) FROM item WHERE path = '' AND is_checked = true"#,)
        .fetch_one(pool)
        .await?;

    Ok((items, total))
}

pub async fn get_label(pool: &SqlitePool, id: i64) -> Result<String, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT label FROM base LEFT JOIN item ON base.id = item.base_id WHERE item.id = ?"#,
        id
    )
    .fetch_one(pool)
    .await
}

pub async fn get_tags(pool: &SqlitePool, id: i64) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT tag.name FROM tag INNER JOIN tag_item ON tag.name = tag_item.tag WHERE tag_item.item = ?",
        id
    )
    .fetch_all(pool)
    .await
}
