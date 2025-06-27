// use sqlx::sqlite::SqliteQueryResult;
// use sqlx::SqlitePool;
// 
// pub struct BasePath{
//     pub id : i64,
//     pub label : String,
// }
// 
// pub async fn mark_all_not_check(pool: &SqlitePool) -> Result<SqliteQueryResult, sqlx::Error> {
//     sqlx::query!(r#"UPDATE base SET is_checked = false"#).execute(pool).await
// }
// 
// pub async fn get(pool: &SqlitePool, id: i64) -> Result<BasePath, sqlx::Error> {
//     sqlx::query_as!(BasePath, "SELECT id, label FROM base WHERE id = ? AND is_checked = true", id).fetch_one(pool).await
// }
// 
// pub async fn get_all(pool: &SqlitePool ) -> Result<Vec<BasePath>, sqlx::Error> {
//     sqlx::query_as!(BasePath, "SELECT id, label FROM base WHERE is_checked = true").fetch_all(pool).await
// }
// 
// pub async fn find_or_create(pool: &SqlitePool, label: &str) -> Result<i64, sqlx::Error> {
//     if let Ok(id) = sqlx::query_scalar!(r#"SELECT id FROM base WHERE label = ?"#, label)
//         .fetch_one(pool)
//         .await
//     {
//         sqlx::query!(r#"UPDATE base SET is_checked = true WHERE id = ?"#, id)
//             .execute(pool)
//             .await?;
//         Ok(id)
//     } else {
//         let id = sqlx::query!(r#"INSERT OR IGNORE INTO base (label) VALUES (?)"#, label)
//             .execute(pool)
//             .await?
//             .last_insert_rowid();
//         Ok(id)
//     }
// }
// 
// pub async fn clean(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
//     let count = sqlx::query!(r#"DELETE FROM base WHERE  is_checked = false"#)
//         .execute(pool)
//         .await?
//         .rows_affected();
//     Ok(count)
// }
