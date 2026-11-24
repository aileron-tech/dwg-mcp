//! HTTP adapter: download a drawing from a URL with a size cap and a time limit.

use std::time::Duration;

use ureq::ResponseExt;

use crate::contract::MAX_FILE_BYTES;
use crate::domain::{download_file_name, has_drawing_extension, HttpUrl, ToolError};

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

/// Fetch the file behind `url`. Redirects are followed; the file name comes from
/// Content-Disposition, then the final URL path, and must end in .dwg or .dxf.
pub fn download(url: &HttpUrl) -> Result<Download, ToolError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(DOWNLOAD_TIMEOUT))
        .user_agent(concat!("dwg-mcp/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();

    let mut response = agent.get(&url.raw).call().map_err(|e| match e {
        ureq::Error::StatusCode(code) => {
            ToolError::DownloadFailed(format!("{} answered HTTP {code}", url.raw))
        }
        other => ToolError::DownloadFailed(format!("{}: {other}", url.raw)),
    })?;

    let header = |name: &str| {
        response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
    };
    let content_type = header("content-type").unwrap_or_default().to_ascii_lowercase();
    if content_type.starts_with("text/html") {
        return Err(ToolError::UnsupportedFile(format!(
            "{} returned a web page (text/html), not a drawing file",
            url.raw
        )));
    }
    if let Some(len) = header("content-length").and_then(|v| v.parse::<u64>().ok()) {
        if len > MAX_FILE_BYTES {
            return Err(ToolError::TooLarge(format!(
                "{} is {len} bytes; the limit is {MAX_FILE_BYTES} bytes",
                url.raw
            )));
        }
    }

    let final_url = crate::domain::parse_http_url(&response.get_uri().to_string())
        .unwrap_or_else(|_| url.clone());
    let file_name = download_file_name(header("content-disposition").as_deref(), &final_url);
    if !has_drawing_extension(&file_name) {
        return Err(ToolError::UnsupportedFile(format!(
            "{} is named {file_name:?}; only .dwg and .dxf files are supported",
            url.raw
        )));
    }

    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_FILE_BYTES)
        .read_to_vec()
        .map_err(|e| match e {
            ureq::Error::BodyExceedsLimit(_) => ToolError::TooLarge(format!(
                "{} is larger than the limit of {MAX_FILE_BYTES} bytes",
                url.raw
            )),
            other => ToolError::DownloadFailed(format!("{}: {other}", url.raw)),
        })?;
    Ok(Download { file_name, bytes })
}
