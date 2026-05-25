use crate::shared::error::{AppError, AppResult};
use super::business::FileInfoResponse;
use reqwest::Client;
use std::path::Path;
use tracing::{debug, error, info};

pub async fn download_telegram_file(
    client: &Client,
    token: &str,
    file_id: &str,
    dest_path: &str,
) -> AppResult<()> {
    debug!("[Telegram.media] Querying file path for file_id: {file_id}");
    let get_file_url = format!("https://api.telegram.org/bot{token}/getFile?file_id={file_id}");

    let res = client.get(&get_file_url).send().await?;
    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        error!("[Telegram.media] Failed to query getFile: HTTP {status} - {err_text}");
        return Err(AppError::Telegram(format!("getFile HTTP {status}: {err_text}")));
    }

    let file_info_resp: FileInfoResponse = res.json().await?;
    if !file_info_resp.ok || file_info_resp.result.is_none() {
        error!("[Telegram.media] getFile response was not ok or has empty result");
        return Err(AppError::Telegram("getFile returned empty result".into()));
    }

    let file_info = file_info_resp.result.unwrap();

    // Enforce strict file size limit (15 MB) to prevent storage depletion DoS
    if let Some(size) = file_info.file_size {
        let max_size = 15 * 1024 * 1024; // 15 MB
        if size > max_size {
            error!("[Telegram.media] File size {size} exceeds max limit of {max_size}");
            return Err(AppError::Telegram(format!(
                "Размер файла ({:.2} МБ) превышает лимит в 15 МБ.",
                size as f64 / 1024.0 / 1024.0
            )));
        }
    }

    let file_path = match file_info.file_path {
        Some(path) => path,
        None => {
            error!("[Telegram.media] getFile returned no file_path");
            return Err(AppError::Telegram("getFile returned no file_path".into()));
        }
    };

    info!("[Telegram.media] Downloading file from Telegram path: {file_path}");
    let download_url = format!("https://api.telegram.org/file/bot{token}/{file_path}");

    let download_res = client.get(&download_url).send().await?;
    if !download_res.status().is_success() {
        let status = download_res.status();
        error!("[Telegram.media] Failed to download file bytes: HTTP {status}");
        return Err(AppError::Telegram(format!("Download file HTTP {status}")));
    }

    let bytes = download_res.bytes().await?;

    // Ensure parent directories exist
    let path = Path::new(dest_path);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            debug!("[Telegram.media] Creating download folder directory: {:?}", parent);
            std::fs::create_dir_all(parent)?;
        }
    }

    // Save bytes to disk
    std::fs::write(dest_path, bytes)?;
    info!("[Telegram.media] File downloaded successfully and saved to: {dest_path}");

    Ok(())
}
