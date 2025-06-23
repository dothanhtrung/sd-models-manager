//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::config::Config;
use jwalk::{Parallelism, WalkDir};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use serde_json::{to_string_pretty, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{error, info};

pub const PREVIEW_EXT: &str = "jpeg";

#[derive(PartialEq)]
enum FileType {
    NA,
    Video,
    Image,
}

pub async fn update_model_info(config: &Config) -> anyhow::Result<()> {
    let valid_ext = config.extensions.iter().collect::<HashSet<_>>();
    let client = Client::new();

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", config.civitai.api_key))?,
    );

    let parallelism = Parallelism::RayonNewPool(config.walkdir_parallel);
    for (_, base_path) in config.model_paths.iter() {
        for entry in WalkDir::new(base_path)
            .skip_hidden(true)
            .parallelism(parallelism.clone())
            .follow_links(true)
            .into_iter()
            .flatten()
        {
            let path = entry.path();
            if entry.file_type().is_file() || entry.file_type().is_symlink() {
                let file_ext = path.extension().unwrap_or_default().to_str().unwrap_or_default();
                if valid_ext.contains(&file_ext.to_string()) {
                    info!("Update model info: {}", entry.path().display());
                    match get_model_info(&path, &client, &headers).await {
                        Ok(info) => {
                            if let Err(e) = save_info(
                                &path,
                                &info,
                                config.civitai.overwrite_thumbnail,
                                &client,
                                &headers,
                            )
                            .await
                            {
                                error!("Failed to save model info: {}", e);
                            }
                        }
                        Err(e) => error!("Failed to download model info: {}", e),
                    }
                }
            }
        }
    }
    Ok(())
}

async fn get_model_info(path: &PathBuf, client: &Client, headers: &HeaderMap) -> anyhow::Result<Value> {
    let hash = calculate_autov2_hash(path)?;
    let url = format!("https://civitai.com/api/v1/model-versions/by-hash/{}", hash);

    let response = client.get(url).headers(headers.clone()).send().await?.json().await?;

    Ok(response)
}

async fn save_info(
    filepath: &PathBuf,
    mode_info: &Value,
    overwrite_thumbnail: bool,
    client: &Client,
    headers: &HeaderMap,
) -> anyhow::Result<()> {
    let mut info_file = filepath.clone();
    info_file.set_extension("json");

    if let Some(images) = mode_info["images"].as_array() {
        if let Some(first_image) = images.first() {
            if let Some(url) = first_image["url"].as_str() {
                let extension = url
                    .split('/')
                    .last()
                    .and_then(|filename| Path::new(filename).extension())
                    .and_then(|ext| ext.to_str())
                    .unwrap_or(PREVIEW_EXT);

                let mut preview_file = filepath.clone();
                preview_file.set_extension(extension);

                let mut saved_file = File::create(info_file)?;
                let info_str = to_string_pretty(mode_info)?;
                saved_file
                    .write_all(info_str.as_bytes())
                    .map_err(|e| anyhow::anyhow!(e))?;

                let image_path = Path::new(&preview_file);
                if image_path.exists() && !overwrite_thumbnail {
                    info!("File already exists: {}", image_path.display());
                    return Ok(());
                } else {
                    let response = client.get(url).headers(headers.clone()).send().await?.bytes().await?;
                    let mut content = response.as_ref();
                    let mut file = File::create(image_path)?;
                    std::io::copy(&mut content, &mut file)?;
                }

                let file_type = file_type(image_path.to_str().unwrap_or_default());
                if file_type == FileType::Video {
                    generate_video_thumbnail(&preview_file, overwrite_thumbnail)?;
                } else if file_type == FileType::Image {
                    //  Change preview image extension to jpeg for easier to manage
                    if image_path.extension().unwrap_or_default() != PREVIEW_EXT {
                        let mut new_name = preview_file.clone();
                        new_name.set_extension(PREVIEW_EXT);
                        fs::rename(preview_file, new_name)?;
                    }
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn calculate_autov2_hash(file_path: &PathBuf) -> std::io::Result<String> {
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

fn generate_video_thumbnail(file_path: &PathBuf, overwrite: bool) -> anyhow::Result<()> {
    let mut thumbnail_path = file_path.clone();
    thumbnail_path.set_extension("jpeg");
    if !overwrite && thumbnail_path.exists() {
        return Ok(());
    }

    Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "quiet",
            "-i",
            file_path.to_str().unwrap_or_default(),
            "-frames",
            "1",
            "-vf",
            r#"select=not(mod(n\,3000)),scale=300:ih*300/iw"#,
            "-q:v",
            "10",
            &thumbnail_path.to_str().unwrap_or_default(),
        ])
        .status()?;

    Ok(())
}

fn file_type(path: &str) -> FileType {
    let data = fs::read(path).ok().unwrap_or_default();
    if let Some(kind) = infer::get(&data) {
        if kind.mime_type().starts_with("video/") {
            return FileType::Video;
        } else if kind.mime_type().starts_with("image/") {
            return FileType::Image;
        }
    }

    FileType::NA
}
