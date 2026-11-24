//! HTTP adapter: download a drawing from a URL with a size cap and a time limit.

use std::net::IpAddr;
use std::time::Duration;

use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

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
/// Content-Disposition, then the final URL path, and must end in .dwg, .dxf or .svg.
pub fn download(url: &HttpUrl) -> Result<Download, ToolError> {
    let agent = ureq::Agent::new_with_config(download_config(false));
    download_with(&agent, url)
}

pub(crate) fn download_public_https(url: &HttpUrl) -> Result<Download, ToolError> {
    let config = download_config(true);
    let agent = ureq::Agent::with_parts(config, DefaultConnector::default(), PublicHttpsResolver);
    download_with(&agent, url)
}

fn download_config(public_https: bool) -> ureq::config::Config {
    let builder = ureq::Agent::config_builder()
        .timeout_global(Some(DOWNLOAD_TIMEOUT))
        .user_agent(concat!("dwg-mcp/", env!("CARGO_PKG_VERSION")));
    if public_https {
        return builder.https_only(true).proxy(None).build();
    }
    builder.build()
}

#[derive(Debug)]
struct PublicHttpsResolver;

impl Resolver for PublicHttpsResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        if uri.scheme_str() != Some("https") || !matches!(uri.port_u16(), None | Some(443)) {
            return Err(ureq::Error::HostNotFound);
        }
        let resolved = DefaultResolver::default().resolve(uri, config, timeout)?;
        let mut public = self.empty();
        // Filter the addresses passed to the connector, including after redirects.
        for &addr in &resolved {
            if is_public_address(addr.ip()) {
                public.push(addr);
            }
        }
        if public.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(public)
    }
}

fn is_public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [first, second, third, _] = ip.octets();
            !ip.is_private()
                && !ip.is_loopback()
                && !ip.is_link_local()
                && !ip.is_documentation()
                && !matches!((first, second, third),
                    (0 | 224..=255, _, _)
                    | (100, 64..=127, _)
                    | (192, 0, 0)
                    | (192, 88, 99)
                    | (198, 18 | 19, _))
        }
        IpAddr::V6(ip) => {
            let [first, second, ..] = ip.segments();
            matches!(first, 0x2000..=0x3fff)
                && !matches!((first, second), (0x2001, 0..=0x1ff | 0xdb8) | (0x2002, _) | (0x3fff, _))
        }
    }
}

fn download_with(agent: &ureq::Agent, url: &HttpUrl) -> Result<Download, ToolError> {
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
            "{} is named {file_name:?}; only .dwg, .dxf and .svg files are supported",
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
