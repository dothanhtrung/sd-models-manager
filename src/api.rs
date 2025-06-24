//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::civitai::{calculate_autov2_hash, update_model_info, PREVIEW_EXT};
use crate::config::Config;
use crate::db::base::find_or_create;
use crate::db::item::update_or_insert;
use crate::db::{base, item, DBPool};
use crate::BASE_PATH_PREFIX;
use actix_web::web::{Data, Query};
use actix_web::{get, web, Responder};
use jwalk::{Parallelism, WalkDir};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use tracing::error;

pub fn scope_config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(get)
            .service(reload_from_disk)
            .service(clean)
            .service(sync_civitai),
    );
}

#[derive(Serialize)]
struct GetResponse {
    items: Vec<ModelInfo>,
    err: Option<String>,
}

#[derive(Serialize)]
struct ReloadResponse {
    count: u64, // Number of item changed
}

#[derive(Serialize)]
struct CleanResponse {
    deleted_items: u64,      // Number of non-existence item deleted
    deleted_base_paths: u64, // Number of non-existence base path deleted
}

#[derive(Deserialize)]
pub struct GetRequest {
    pub folder: Option<i64>,
    pub page: Option<i64>,
    pub count: Option<i64>,
}

#[derive(Serialize)]
struct ModelInfo {
    id: i64,
    name: String,
    info: String,
    preview: String,
    is_dir: bool,
}

#[get("get")]
async fn get(config: Data<Config>, db_pool: Data<DBPool>, query_params: Query<GetRequest>) -> impl Responder {
    let page = query_params.page.unwrap_or(0);
    let limit = query_params.count.unwrap_or(config.count);
    let offset = page * limit;
    let mut ret = Vec::new();
    let mut err = None;
    if let Some(folder) = query_params.folder {
        let Ok(base_label) = item::get_label(&db_pool.sqlite_pool, folder).await else {
            return web::Json(GetResponse {
                items: ret,
                err: Some("Cannot find model path".to_string()),
            });
        };

        let Some(base_path) = config.model_paths.get(&base_label) else {
            return web::Json(GetResponse {
                items: ret,
                err: Some("Unknown model path for folder".to_string()),
            });
        };

        let base_path = PathBuf::from(base_path);

        match item::get(&db_pool.sqlite_pool, folder, limit, offset).await {
            Ok(items) => {
                for item in items {
                    let model_path = base_path.join(&item.path);
                    let name = model_path
                        .file_name()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default()
                        .to_string();

                    let mut info = String::new();
                    let mut preview = String::from("/assets/folder.png");
                    let mut is_dir = false;

                    if model_path.is_dir() {
                        is_dir = true;
                    } else {
                        let mut info_path = model_path.clone();
                        info_path.set_extension("json");
                        info = fs::read_to_string(info_path).unwrap_or_default();

                        let preview_path = PathBuf::from(format!("/{}{}", BASE_PATH_PREFIX, base_label));
                        let mut preview_path = preview_path.join(item.path);
                        preview_path.set_extension(PREVIEW_EXT);
                        preview = preview_path.to_str().unwrap_or_default().to_string();
                    }

                    ret.push(ModelInfo {
                        id: item.id,
                        name,
                        info,
                        preview,
                        is_dir,
                    })
                }
            }
            Err(e) => err = Some(format!("{}", e)),
        }
    } else {
        match item::get_root(&db_pool.sqlite_pool, limit, offset).await {
            Ok(items) => {
                for item in items {
                    let model_path = PathBuf::from(&item.path);
                    let name = model_path
                        .file_name()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default()
                        .to_string();
                    ret.push(ModelInfo {
                        id: item.id,
                        name,
                        info: String::new(),
                        preview: String::new(),
                        is_dir: true,
                    })
                }
            }
            Err(e) => err = Some(format!("{}", e)),
        }
    }

    web::Json(GetResponse { items: ret, err })
}

#[get("reload_from_disk")]
async fn reload_from_disk(config: Data<Config>, db_pool: Data<DBPool>) -> impl Responder {
    let valid_ext = config.extensions.iter().collect::<HashSet<_>>();
    let mut count = 0;

    if let Err(e) = item::mark_all_not_check(&db_pool.sqlite_pool).await {
        error!("Failed to mark all item for reload: {}", e);
        return web::Json(ReloadResponse { count });
    }

    if let Err(e) = base::mark_all_not_check(&db_pool.sqlite_pool).await {
        error!("Failed to mark all item for reload: {}", e);
        return web::Json(ReloadResponse { count });
    }

    for (label, base_path) in config.model_paths.iter() {
        let Ok(base_id) = find_or_create(&db_pool.sqlite_pool, label).await else {
            continue;
        };

        let parallelism = Parallelism::RayonNewPool(config.walkdir_parallel);
        for entry in WalkDir::new(base_path)
            .skip_hidden(true)
            .parallelism(parallelism.clone())
            .follow_links(true)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() || entry.file_type().is_symlink() {
                let path = entry.path();
                let Ok(relative_path) = get_relative_path(base_path, &path) else {
                    continue;
                };
                let file_ext = path.extension().unwrap_or_default().to_str().unwrap_or_default();
                if valid_ext.contains(&file_ext.to_string()) {
                    let hash = calculate_autov2_hash(&path).unwrap_or_default();
                    if let Err(e) = update_or_insert(&db_pool.sqlite_pool, hash.as_str(), &relative_path, base_id).await
                    {
                        error!("Failed to insert item: {}", e);
                    } else {
                        count += 1;
                    }
                }
            }
        }
    }

    web::Json(ReloadResponse { count })
}

#[get("clean")]
async fn clean(db_pool: Data<DBPool>) -> impl Responder {
    let deleted_items = item::clean(&db_pool.sqlite_pool).await.unwrap_or_default();
    let deleted_base_paths = base::clean(&db_pool.sqlite_pool).await.unwrap_or_default();

    web::Json(CleanResponse {
        deleted_items,
        deleted_base_paths,
    })
}

#[get("sync_civitai")]
async fn sync_civitai(config: Data<Config>) -> impl Responder {
    let _ = update_model_info(&**config).await;
    web::Json("")
}

fn get_relative_path(base_path: &str, path: &PathBuf) -> Result<String, anyhow::Error> {
    let base = PathBuf::from(base_path);
    let path = path.strip_prefix(&base)?;
    Ok(path.to_str().unwrap_or_default().to_string())
}
