//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use actix_files::Files;
use actix_web::web::Data;
use actix_web::{error, get, web, HttpResponse, Responder};
use serde::Deserialize;
use tera::Tera;

pub fn scope_config(cfg: &mut web::ServiceConfig) {
    let tera = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/res/html/**/*")).unwrap();
    cfg.app_data(Data::new(tera))
        .service(Files::new("/assets", concat!(env!("CARGO_MANIFEST_DIR"), "/res/assets")))
        .service(Files::new("/css", concat!(env!("CARGO_MANIFEST_DIR"), "/res/css")))
        .service(Files::new("/js", concat!(env!("CARGO_MANIFEST_DIR"), "/res/js")))
        .service(web::scope("/").service(index));
}

#[get("")]
async fn index(tmpl: Data<Tera>) -> impl Responder {
    let mut ctx = tera::Context::new();
    let template = tmpl
        .render("index.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(format!("Template error: {:?}", e)))
        .unwrap();
    HttpResponse::Ok().content_type("text/html").body(template)
}
