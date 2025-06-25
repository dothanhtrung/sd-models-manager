//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::civitai::{update_model_info, PREVIEW_EXT};
use crate::config::Config;
use crate::db::base::find_or_create;
use crate::db::item::{update_or_insert, Item};
use crate::db::{base, item, DBPool};
use crate::BASE_PATH_PREFIX;
use actix_web::web::{Data, Query};
use actix_web::{get, rt, web, Responder};
use jwalk::{Parallelism, WalkDir};
use serde::{Deserialize, Serialize};
use sqlx::Error;
use std::cmp::max;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::fs;
use tracing::error;

const TRASH_DIR: &str = ".trash";

pub fn scope_config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(get)
            .service(get_item)
            .service(reload_from_disk)
            .service(clean)
            .service(delete)
            .service(empty_trash)
            .service(search)
            .service(sync_civitai),
    );
}

#[derive(Serialize)]
struct GetResponse {
    items: Vec<ModelInfo>,
    total: i64,
    err: Option<String>,
}

#[derive(Deserialize)]
struct GetRequest {
    pub folder: Option<i64>,
    pub page: Option<i64>,
    pub count: Option<i64>,
}

#[derive(Serialize, Default)]
struct ModelInfo {
    id: i64,
    name: String,
    path: String,
    preview: String,
    is_dir: bool,
    info: Option<String>,
}

#[derive(Deserialize)]
struct DeleteRequest {
    id: Vec<i64>,
}

#[get("get")]
async fn get(config: Data<Config>, db_pool: Data<DBPool>, query_params: Query<GetRequest>) -> impl Responder {
    let page = max(1, query_params.page.unwrap_or(1)) - 1;
    let limit = max(0, query_params.count.unwrap_or(config.api.per_page as i64));
    let offset = page * limit;
    let mut ret = Vec::new();
    let mut err = None;
    let mut total = 0;

    if let Some(folder) = query_params.folder {
        let Ok(base_label) = item::get_label(&db_pool.sqlite_pool, folder).await else {
            return web::Json(GetResponse {
                items: ret,
                total: 0,
                err: Some("Cannot find model path".to_string()),
            });
        };

        let Some(base_path) = config.model_paths.get(&base_label) else {
            return web::Json(GetResponse {
                items: ret,
                total: 0,
                err: Some("Unknown model path for folder".to_string()),
            });
        };

        let base_path = PathBuf::from(base_path);

        match item::get(&db_pool.sqlite_pool, folder, limit, offset).await {
            Ok((items, _total)) => {
                total = _total;
                for item in items {
                    let model_path = base_path.join(&item.path);

                    let mut preview = String::from("/assets/folder.png");
                    let mut is_dir = false;

                    if model_path.is_dir() || item.path.is_empty() {
                        is_dir = true;
                    } else {
                        let preview_path = PathBuf::from(format!("/{}{}", BASE_PATH_PREFIX, base_label));
                        let mut preview_path = preview_path.join(item.path);
                        preview_path.set_extension(PREVIEW_EXT);
                        preview = preview_path.to_str().unwrap_or_default().to_string();
                    }

                    ret.push(ModelInfo {
                        id: item.id,
                        name: item.name.unwrap_or_default(),
                        path: model_path.to_str().unwrap_or_default().to_string(),
                        preview,
                        is_dir,
                        info: None,
                    })
                }
            }
            Err(e) => err = Some(format!("{}", e)),
        }
    } else {
        match item::get_root(&db_pool.sqlite_pool, limit, offset).await {
            Ok((items, _total)) => {
                total = _total;
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
                        path: model_path.to_str().unwrap_or_default().to_string(),
                        preview: String::from("/assets/folder.png"),
                        is_dir: true,
                        info: None,
                    })
                }
            }
            Err(e) => err = Some(format!("{}", e)),
        }
    }

    web::Json(GetResponse { items: ret, total, err })
}

#[get("item/{id}")]
async fn get_item(config: Data<Config>, db_pool: Data<DBPool>, url_param: web::Path<(i64,)>) -> impl Responder {
    let item_id = url_param.into_inner().0;
    match item::get_by_id(&db_pool.sqlite_pool, item_id).await {
        Ok((_item, label)) => {
            let (model_path, json_path, preview_path) = get_abs_path(&config, &label, &_item.path);
            let info = fs::read_to_string(&json_path).await.unwrap_or_default();
            let is_dir = PathBuf::from(&model_path).is_dir();
            let item = ModelInfo {
                id: item_id,
                name: _item.name.unwrap_or_default(),
                path: model_path,
                preview: preview_path,
                is_dir,
                info: Some(info),
            };
            web::Json(GetResponse {
                items: vec![item],
                total: 1,
                err: None,
            })
        }
        Err(e) => web::Json(GetResponse {
            items: Vec::new(),
            total: 0,
            err: Some(e.to_string()),
        }),
    }
}

#[get("reload_from_disk")]
async fn reload_from_disk(config: Data<Config>, db_pool: Data<DBPool>) -> impl Responder {
    rt::spawn(async move {
        let valid_ext = config.extensions.iter().collect::<HashSet<_>>();

        if let Err(e) = item::mark_all_not_check(&db_pool.sqlite_pool).await {
            error!("Failed to mark all item for reload: {}", e);
            return;
        }

        if let Err(e) = base::mark_all_not_check(&db_pool.sqlite_pool).await {
            error!("Failed to mark all item for reload: {}", e);
            return;
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
                let mut need_update = false;
                let path = entry.path();

                let mut name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                    .to_string();

                let Ok(relative_path) = get_relative_path(base_path, &path) else {
                    continue;
                };

                if entry.file_type().is_file() || entry.file_type().is_symlink() {
                    let file_ext = path.extension().unwrap_or_default().to_str().unwrap_or_default();
                    if valid_ext.contains(&file_ext.to_string()) {
                        need_update = true;
                    }
                } else {
                    need_update = true;
                    if relative_path.is_empty() {
                        name = (*label).clone();
                    }
                }

                if need_update {
                    if let Err(e) =
                        update_or_insert(&db_pool.sqlite_pool, Some(name.as_str()), &relative_path, base_id).await
                    {
                        error!("Failed to insert item: {}", e);
                    }
                }
            }
        }
    });
    web::Json("")
}

#[get("clean")]
async fn clean(db_pool: Data<DBPool>) -> impl Responder {
    let deleted_items = item::clean(&db_pool.sqlite_pool).await.unwrap_or_default();
    let deleted_base_paths = base::clean(&db_pool.sqlite_pool).await.unwrap_or_default();

    web::Json(format!(
        "{{
        \"deleted_items\": {},
        \"deleted_base_paths\": {},
    }}",
        deleted_items, deleted_base_paths,
    ))
}

#[get("sync_civitai")]
async fn sync_civitai(config: Data<Config>) -> impl Responder {
    let config = (**config).clone();
    rt::spawn(async { update_model_info(config).await });
    web::Json("")
}

#[get("delete")]
async fn delete(config: Data<Config>, db_pool: Data<DBPool>, params: Query<DeleteRequest>) -> impl Responder {
    for id in params.id.iter() {
        let Ok((rel_path, label)) = item::mark_obsolete(&db_pool.sqlite_pool, *id).await else {
            continue;
        };
        let Some(base_path) = config.model_paths.get(&label) else {
            continue;
        };
        let base_path = PathBuf::from(base_path);
        let model_file = base_path.join(rel_path);
        let trash_dir = base_path.join(TRASH_DIR);

        let mut json_file = model_file.clone();
        json_file.set_extension("json");
        let mut preview_file = model_file.clone();
        preview_file.set_extension(PREVIEW_EXT);
        // TODO: Removed downloaded video

        for file in [model_file, json_file, preview_file].iter() {
            if let Err(e) = move_to_dir(file, &trash_dir).await {
                error!("Failed to move file to trash directory: {}", e);
            }
        }
    }

    web::Json("")
}

#[get("empty_trash")]
async fn empty_trash(config: Data<Config>) -> impl Responder {
    for (_, base_path) in config.model_paths.iter() {
        let trash_dir = PathBuf::from(base_path).join(TRASH_DIR);
        if let Err(e) = fs::remove_dir_all(&trash_dir).await {
            error!("Failed to remove trash directory: {}", e);
        }
    }
    web::Json("")
}

#[get("search")]
async fn search() -> impl Responder {
    web::Json("")
}

async fn move_to_dir(file: &PathBuf, dir: &PathBuf) -> anyhow::Result<()> {
    let file_name = file.file_name().unwrap_or_default();
    if !file_name.is_empty() {
        let dest = dir.join(file_name);
        fs::rename(file, dest).await?;
    }

    Ok(())
}

fn get_relative_path(base_path: &str, path: &PathBuf) -> Result<String, anyhow::Error> {
    let base = PathBuf::from(base_path);
    let path = path.strip_prefix(&base)?;
    Ok(path.to_str().unwrap_or_default().to_string())
}

/// Return abs path of (model, json) and http path of preview
fn get_abs_path(config: &Config, label: &str, rel_path: &str) -> (String, String, String) {
    let (mut model, mut json, mut preview) = (String::new(), String::new(), String::new());
    if let Some(base_path) = config.model_paths.get(label) {
        let base_path = PathBuf::from(base_path);
        let model_path = base_path.join(rel_path);
        model = model_path.to_str().unwrap_or_default().to_string();

        let mut json_path = model_path.clone();
        json_path.set_extension("json");
        json = json_path.to_str().unwrap_or_default().to_string();

        let img_path = PathBuf::from(format!("/{}{}", BASE_PATH_PREFIX, label));
        let mut preview_path = img_path.join(rel_path);
        preview_path.set_extension(PREVIEW_EXT);
        preview = preview_path.to_str().unwrap_or_default().to_string();
    }

    (model, json, preview)
}
