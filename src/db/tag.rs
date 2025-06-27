use sqlx::SqlitePool;

pub async fn add_tag(pool: &SqlitePool, name: &str) -> anyhow::Result<()> {
    sqlx::query!("INSERT OR IGNORE INTO tag (name) VALUES (?)", name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_tag(pool: &SqlitePool, name: &str) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM tag WHERE name = ?", name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rename_tag(pool: &SqlitePool, name: &str, new_name: &str) -> anyhow::Result<()> {
    if let Ok(exist_name) = sqlx::query_scalar!("SELECT name FROM tag WHERE name = ?", new_name)
        .fetch_one(pool)
        .await
    {
        return Err(anyhow::anyhow!("{} already exists", exist_name));
    }

    sqlx::query!("UPDATE tag SET name = ? WHERE name = ?", new_name, name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn add_tag_item(pool: &SqlitePool, item: i64, tags: Vec<&str>) -> Result<(), sqlx::Error> {
    for tag in tags {
        sqlx::query!(
            "INSERT OR IGNORE INTO tag_item (item, tag) VALUES (?, (SELECT id FROM tag WHERE name = ?))",
            item,
            tag
        )
        .execute(pool)
        .await?;
    }
    // TODO: Insert depend tags

    Ok(())
}

pub async fn remove_tag_item(pool: &SqlitePool, item: i64, tag: &str) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM tag_item WHERE item = ? AND tag = ?", item, tag)
        .execute(pool)
        .await?;
    Ok(())
}
