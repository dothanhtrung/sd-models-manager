//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::config::Config;
use jwalk::{DirEntry, Parallelism, WalkDir, WalkDirGeneric};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use serde_json::{to_string_pretty, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use tracing::{error, info};
use tracing_subscriber::fmt::format::Pretty;

pub async fn update_model_info(config: &Config) -> anyhow::Result<()> {
    let valid_ext = config.extensions.iter().collect::<HashSet<_>>();
    let client = Client::new();

    let mut headers = HeaderMap::new();
    // headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", config.civitai.api_key))?,
    );

    let parallelism = Parallelism::RayonNewPool(config.walkdir_parallel);
    for base_path in config.model_paths.iter() {
        for entry in WalkDir::new(base_path)
            .skip_hidden(true)
            .parallelism(parallelism.clone())
            .follow_links(true)
        {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type().is_file() || entry.file_type().is_symlink() {
                let file_ext = path.extension().unwrap_or_default().to_str().unwrap_or_default();
                if valid_ext.contains(&file_ext.to_string()) {
                    info!("Update model info: {}", entry.path().display());
                    if let Some(path) = entry.path().to_str() {
                        match get_model_info(path, &client, &headers).await {
                            Ok(info) => {
                                download_info(
                                    file_ext,
                                    path,
                                    &info,
                                    config.civitai.overwrite_thumbnail,
                                    &client,
                                    &headers,
                                )
                                .await?
                            }
                            Err(e) => error!("Failed to download model info: {}", e),
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

async fn get_model_info(path: &str, client: &Client, headers: &HeaderMap) -> anyhow::Result<Value> {
    let hash = calculate_autov2_hash(path)?;
    let url = format!("https://civitai.com/api/v1/model-versions/by-hash/{}", hash);

    let response = client.get(url).headers(headers.clone()).send().await?.json().await?;

    Ok(response)
}

async fn download_info(
    ext: &str,
    filepath: &str,
    mode_info: &Value,
    overwrite_thumbnail: bool,
    client: &Client,
    headers: &HeaderMap,
) -> anyhow::Result<()> {
    let info_file = filepath.replace(ext, "json");

    if let Some(images) = mode_info["images"].as_array() {
        if let Some(first_image) = images.first() {
            if let Some(url) = first_image["url"].as_str() {
                let extension = url
                    .split('/')
                    .last()
                    .and_then(|filename| Path::new(filename).extension())
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("jpeg");

                let image_file = filepath.replace(ext, extension);

                let mut saved_file = File::create(info_file)?;
                let info_str = to_string_pretty(mode_info)?;
                saved_file
                    .write_all(info_str.as_bytes())
                    .map_err(|e| anyhow::anyhow!(e))?;

                let image_path = Path::new(&image_file);
                if image_path.exists() && !overwrite_thumbnail {
                    info!("File already exists: {}", image_path.display());
                    return Ok(());
                }

                let response = client.get(url).headers(headers.clone()).send().await?.bytes().await?;
                let mut content = response.as_ref();
                let mut file = File::create(image_path)?;
                std::io::copy(&mut content, &mut file)?;
            }
        }
    }

    Ok(())
}

fn calculate_autov2_hash(file_path: &str) -> std::io::Result<String> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(hex::encode(result)[..10].to_string())
}
