//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::civitai::{update_model_info, CivitaiFileMetadata, CivitaiModel, PREVIEW_EXT};
use crate::config::Config;
use crate::db::item::{insert_or_update, Item};
use crate::db::tag::add_tag_from_model_info;
use crate::db::{base, item, DBPool};
use crate::BASE_PATH_PREFIX;
use actix_web::web::{Data, Query};
use actix_web::{get, rt, web, Responder};
use jwalk::{Parallelism, WalkDir};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    pub page: Option<i64>,
    pub count: Option<i64>,
    pub search: Option<String>,
    pub tags: Option<String>,
}

#[derive(Serialize, Default)]
struct ModelInfo {
    id: i64,
    name: String,
    path: String,
    preview: String,
    info: Option<String>,
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct DeleteRequest {
    id: Vec<i64>,
}

#[get("")]
async fn get(config: Data<Config>, db_pool: Data<DBPool>, query_params: Query<GetRequest>) -> impl Responder {
    let page = max(1, query_params.page.unwrap_or(1)) - 1;
    let limit = max(0, query_params.count.unwrap_or(config.api.per_page as i64));
    let offset = page * limit;
    let mut ret = Vec::new();
    let mut err = None;
    let mut total = 0;
    let mut items = Vec::new();
    (items, total) = if let Some(search_string) = &query_params.search {
        match item::search(&db_pool.sqlite_pool, search_string, limit, offset).await {
            Ok((i, t)) => (i, t),
            Err(e) => {
                err = Some(format!("{}", e));
                (Vec::new(), 0)
            }
        }
    } else {
        match item::get(&db_pool.sqlite_pool, limit, offset).await {
            Ok((i, t)) => (i, t),
            Err(e) => {
                err = Some(format!("{}", e));
                (Vec::new(), 0)
            }
        }
    };

    for item in items {
        let (model_url, _, preview_url) = get_abs_path(&config, &item.base_label, &item.path);

        let tags = item::get_tags(&db_pool.sqlite_pool, item.id).await.unwrap_or_default();

        ret.push(ModelInfo {
            id: item.id,
            name: item.name.unwrap_or_default(),
            path: model_url,
            preview: preview_url,
            tags,
            info: None,
        })
    }

    web::Json(GetResponse { items: ret, total, err })
}

#[get("item/{id}")]
async fn get_item(config: Data<Config>, db_pool: Data<DBPool>, url_param: web::Path<(i64,)>) -> impl Responder {
    let item_id = url_param.into_inner().0;
    match item::get_by_id(&db_pool.sqlite_pool, item_id).await {
        Ok(_item) => {
            let (model_url, json_url, preview_url) = get_abs_path(&config, &_item.base_label, &_item.path);
            let info = fs::read_to_string(&json_url).await.unwrap_or_default();
            let tags = item::get_tags(&db_pool.sqlite_pool, item_id).await.unwrap_or_default();
            let item = ModelInfo {
                id: item_id,
                name: _item.name.unwrap_or_default(),
                path: model_url,
                preview: preview_url,
                tags,
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

        if let Err(e) = item::mark_obsolete_all(&db_pool.sqlite_pool).await {
            error!("Failed to mark all item for reload: {}", e);
            return;
        }

        for (label, base_path) in config.model_paths.iter() {
            let parallelism = Parallelism::RayonNewPool(config.walkdir_parallel);
            for entry in WalkDir::new(base_path)
                .skip_hidden(true)
                .parallelism(parallelism.clone())
                .follow_links(true)
                .into_iter()
                .flatten()
            {
                let path = entry.path();

                let name = path
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
                        let mut json_file = PathBuf::from(path);
                        json_file.set_extension("json");
                        let info = fs::read_to_string(&json_file).await.unwrap_or_default();
                        let v: Value = serde_json::from_str(&info).unwrap();
                        let blake3 = v["files"][0]["hashes"]["BLAKE3"].as_str().unwrap_or_default();
                        let file_metadata =
                            serde_json::from_value::<CivitaiFileMetadata>(v["files"][0]["metadata"].clone())
                                .unwrap_or_default();
                        let model_info = serde_json::from_value::<CivitaiModel>(v["model"].clone()).unwrap_or_default();

                        match insert_or_update(
                            &db_pool.sqlite_pool,
                            Some(name.as_str()),
                            &relative_path,
                            label,
                            blake3,
                            &model_info.name,
                        )
                        .await
                        {
                            Ok(id) => {
                                if let Err(e) =
                                    add_tag_from_model_info(&db_pool.sqlite_pool, id, &model_info, &file_metadata).await
                                {
                                    error!("Failed to insert tag: {}", e);
                                }
                            }
                            Err(e) => error!("Failed to insert item: {}", e),
                        }
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

    web::Json(format!(
        "{{
        \"deleted_items\": {},
    }}",
        deleted_items,
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
