use crate::civitai::{CivitaiFileMetadata, CivitaiModel};
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

pub async fn add_tag_item(pool: &SqlitePool, item: i64, tags: &Vec<String>) -> Result<(), sqlx::Error> {
    for tag in tags {
        let tag_id = match sqlx::query_scalar!("SELECT id FROM tag WHERE name = ?", tag)
            .fetch_one(pool)
            .await
        {
            Ok(id) => id,
            Err(_) => sqlx::query!("INSERT INTO tag (name) VALUES (?)", tag)
                .execute(pool)
                .await?
                .last_insert_rowid(),
        };
        sqlx::query!("INSERT OR IGNORE INTO tag_item (item, tag) VALUES (?, ?)", item, tag_id)
            .execute(pool)
            .await?;
    }
    // TODO: Insert depend tags

    Ok(())
}

pub async fn add_tag_from_model_info(
    pool: &SqlitePool,
    item: i64,
    extra_tags: &Vec<String>,
    model_info: &CivitaiModel,
    file_metadata: &CivitaiFileMetadata,
) -> Result<(), sqlx::Error> {
    let mut tags = Vec::new();
    for tag in extra_tags {
        tags.push(tag.clone().replace(" ", "_").to_lowercase());
    }

    tags.push(model_info.model_type.clone().replace(" ", "_").to_lowercase());
    if model_info.nsfw {
        tags.push(String::from("nsfw"));
    }
    if model_info.poi {
        tags.push(String::from("poi"));
    }
    tags.push(file_metadata.format.clone().replace(" ", "_").to_lowercase());
    if let Some(fp) = file_metadata.fp {
        tags.push(fp.to_string());
    }
    add_tag_item(pool, item, &tags).await
}

pub async fn remove_tag_item(pool: &SqlitePool, item: i64, tag: &str) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM tag_item WHERE item = ? AND tag = ?", item, tag)
        .execute(pool)
        .await?;
    Ok(())
}
