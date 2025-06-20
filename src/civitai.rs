//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::config::Config;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use tracing::{error, info};
use tracing_subscriber::fmt::format::Pretty;
use walkdir::{DirEntry, WalkDir};

fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name().to_str().map(|s| s.starts_with(".")).unwrap_or(false)
}

pub async fn update_model_info(config: &Config) -> anyhow::Result<()> {
    for base_path in config.model_paths.iter() {
        for entry in WalkDir::new(base_path)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let file_ext = entry
                    .path()
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default();
                for ext in config.extensions.iter() {
                    if ext == file_ext {
                        info!("Update model info: {}", entry.path().display());
                        if let Some(path) = entry.path().to_str() {
                            match get_model_info(path, config.civitai.api_key.as_str()).await {
                                Ok(info) => {
                                    download_info(
                                        ext,
                                        path,
                                        &info,
                                        config.civitai.overwrite_thumbnail,
                                        config.civitai.api_key.as_str(),
                                    )
                                    .await?
                                }
                                Err(e) => error!("Failed to download model info: {}", e),
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn get_model_info(path: &str, api_key: &str) -> anyhow::Result<CivitaiResponse> {
    let hash = calculate_autov2_hash(path)?;
    let url = format!("https://civitai.com/api/v1/model-versions/by-hash/{}", hash);

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", api_key))?);

    let client = Client::new();
    let response = client
        .get(url)
        .headers(headers)
        .send()
        .await?
        .json::<CivitaiResponse>()
        .await?;

    Ok(response)
}

async fn download_info(
    ext: &str,
    filepath: &str,
    mode_info: &CivitaiResponse,
    overwrite_thumbnail: bool,
    api_key: &str,
) -> anyhow::Result<()> {
    let info_file = filepath.replace(ext, "json");
    let image_file = filepath.replace(ext, "jpeg");

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

    if let Some(first_image) = mode_info.images.first() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", api_key))?);
        let client = Client::new();
        let response = client
            .get(first_image.url.as_str())
            .headers(headers)
            .send()
            .await?
            .bytes()
            .await?;
        let mut content = response.as_ref();
        let mut file = File::create(image_path)?;
        std::io::copy(&mut content, &mut file)?;
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

#[derive(Serialize, Deserialize)]
struct Meta {
    pub seed: i64,
    pub vaes: Vec<String>,
    pub comfy: String,
    pub steps: i64,
    pub width: i64,
    pub height: i64,
    pub models: Vec<String>,
    pub prompt: String,
    pub denoise: i64,
    pub sampler: String,
    #[serde(rename = "cfgScale")]
    pub cfg_scale: f64,
    #[serde(rename = "modelIds")]
    pub model_ids: Vec<u64>,
    pub scheduler: String,
    pub upscalers: Vec<String>,
    #[serde(rename = "versionIds")]
    pub version_ids: Vec<String>,
    #[serde(rename = "controlNets")]
    pub control_nets: Vec<String>,
    #[serde(rename = "additionalResources")]
    pub additional_resources: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ImageMetadata {
    pub hash: String,
    pub size: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Serialize, Deserialize)]
struct CivitaiImage {
    pub url: String,
    #[serde(rename = "nsfwLevel")]
    pub nsfw_level: i64,
    pub width: i64,
    pub height: i64,
    pub hash: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub metadata: ImageMetadata,
    pub minor: bool,
    pub poi: bool,
    pub meta: Meta,
    pub availability: String,
    #[serde(rename = "hasMeta")]
    pub has_meta: bool,
    #[serde(rename = "hasPositivePrompt")]
    pub has_positive_prompt: bool,
    #[serde(rename = "onSite")]
    pub on_site: bool,
    #[serde(rename = "remixOfId")]
    pub remix_of_id: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct Hashes {
    #[serde(rename = "AutoV1")]
    pub auto_v1: String,
    #[serde(rename = "AutoV2")]
    pub auto_v2: String,
    #[serde(rename = "SHA256")]
    pub sha256: String,
    #[serde(rename = "CRC32")]
    pub crc32: String,
    #[serde(rename = "BLAKE3")]
    pub blake3: String,
    #[serde(rename = "AutoV3")]
    pub auto_v3: String,
}

#[derive(Serialize, Deserialize)]
struct FileMetadata {
    pub format: String,
    pub size: Option<u64>,
    pub fp: Option<u32>,
}

#[derive(Serialize, Deserialize)]
struct CivitaiFile {
    pub id: i64,
    #[serde(rename = "sizeKB")]
    pub size_kb: f64,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(rename = "pickleScanResult")]
    pub pickle_scan_result: String,
    #[serde(rename = "pickleScanMessage")]
    pub pickle_scan_message: String,
    #[serde(rename = "virusScanResult")]
    pub virus_scan_result: String,
    #[serde(rename = "virusScanMessage")]
    pub virus_scan_message: Option<String>,
    #[serde(rename = "scannedAt")]
    pub scanned_at: String,
    pub metadata: FileMetadata,
    pub hashes: Hashes,
    pub primary: bool,
    #[serde(rename = "downloadUrl")]
    pub download_url: String,
}

#[derive(Serialize, Deserialize)]
struct Model {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub nsfw: bool,
    pub poi: bool,
}

#[derive(Serialize, Deserialize)]
struct Stats {
    #[serde(rename = "downloadCount")]
    pub download_count: i64,
    #[serde(rename = "ratingCount")]
    pub rating_count: i64,
    pub rating: i64,
    #[serde(rename = "thumbsUpCount")]
    pub thumbs_up_count: i64,
}

#[derive(Serialize, Deserialize)]
struct CivitaiResponse {
    pub id: i64,
    #[serde(rename = "modelId")]
    pub model_id: i64,
    pub name: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub status: String,
    #[serde(rename = "publishedAt")]
    pub published_at: String,
    #[serde(rename = "trainedWords")]
    pub trained_words: Vec<String>,
    #[serde(rename = "trainingStatus")]
    pub training_status: Option<String>,
    #[serde(rename = "trainingDetails")]
    pub training_details: Option<String>,
    #[serde(rename = "baseModel")]
    pub base_model: String,
    #[serde(rename = "baseModelType")]
    pub base_model_type: Option<String>,
    #[serde(rename = "earlyAccessEndsAt")]
    pub early_access_ends_at: Option<String>,
    #[serde(rename = "earlyAccessConfig")]
    pub early_access_config: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "uploadType")]
    pub upload_type: String,
    #[serde(rename = "usageControl")]
    pub usage_control: String,
    pub air: String,
    pub stats: Stats,
    pub model: Model,
    pub files: Vec<CivitaiFile>,
    pub images: Vec<CivitaiImage>,
    #[serde(rename = "downloadUrl")]
    pub download_url: String,
}
