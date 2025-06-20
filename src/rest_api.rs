//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use actix_files::Files;
use actix_web::{post, web, Responder};
use actix_web::web::Data;
use serde::{Deserialize, Serialize};
use crate::ui::index;

pub fn scope_config(cfg: &mut web::ServiceConfig) {
    cfg
        .service(web::scope("/api").service(get));
}

#[derive(Deserialize)]
struct GetRequest {
    path: String,
    page: Option<usize>,
}

#[derive(Serialize)]
struct ModelInfo {
    path: String,
    real_path: String,
    info: String,
}

#[derive(Serialize)]
struct GetResponse {
    files: Vec<ModelInfo>
}

#[post("/get")]
async fn get() -> impl Responder {
    web::Json("")
}