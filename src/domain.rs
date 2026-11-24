//! Pure rules of the MCP server: validation, cache keys, expiry, cost, paging.
//! No I/O and no clock: every time is passed in as Unix seconds.

use std::fmt;

use crate::contract::{
    Label, CACHE_TTL_SECS, COST_CACHED_PARSE, COST_FRESH_PARSE, JOBS_LIMIT_DEFAULT,
    JOBS_LIMIT_MAX, LABELS_LIMIT_DEFAULT, LABELS_LIMIT_MAX, SIZE_DEFAULT, SIZE_MAX, SIZE_MIN,
};

/// Every way a tool call can fail. The code is the stable part of the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    InvalidArgument(String),
    NotFound(String),
    UnsupportedFile(String),
    TooLarge(String),
    DownloadFailed(String),
    ParseFailed(String),
    Internal(String),
}

impl ToolError {
    pub fn code(&self) -> &'static str {
        match self {
            ToolError::InvalidArgument(_) => "invalid_argument",
            ToolError::NotFound(_) => "not_found",
            ToolError::UnsupportedFile(_) => "unsupported_file",
            ToolError::TooLarge(_) => "too_large",
            ToolError::DownloadFailed(_) => "download_failed",
            ToolError::ParseFailed(_) => "parse_failed",
            ToolError::Internal(_) => "internal",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            ToolError::InvalidArgument(m)
            | ToolError::NotFound(m)
            | ToolError::UnsupportedFile(m)
            | ToolError::TooLarge(m)
            | ToolError::DownloadFailed(m)
            | ToolError::ParseFailed(m)
            | ToolError::Internal(m) => m,
        }
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

/// Render options that decide a cache entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    pub size: u32,
    pub tiles: bool,
}

pub fn resolve_options(size: Option<u32>, tiles: Option<bool>) -> Result<ParseOptions, ToolError> {
    let size = size.unwrap_or(SIZE_DEFAULT);
    if !(SIZE_MIN..=SIZE_MAX).contains(&size) {
        return Err(ToolError::InvalidArgument(format!(
            "size must be between {SIZE_MIN} and {SIZE_MAX}, got {size}"
        )));
    }
    Ok(ParseOptions { size, tiles: tiles.unwrap_or(true) })
}

pub fn has_dwg_extension(name: &str) -> bool {
    name.len() > 4 && name.to_ascii_lowercase().ends_with(".dwg")
}

/// A DWG starts with its version code, "AC" and four digits.
pub fn has_dwg_magic(head: &[u8]) -> bool {
    head.len() >= 6 && &head[..2] == b"AC" && head[2..6].iter().all(u8::is_ascii_digit)
}

const FAILSAFE_MARKER: &str = "Failsafe: file header read failed";

/// Why a drawing counts as unreadable, if it does. The parser's failsafe mode turns a
/// damaged file into an empty document instead of an error; a document with no entities
/// and a failed header read is that case, not a drawing.
pub fn unreadable_reason(notifications: &[String], entity_count: usize) -> Option<String> {
    if entity_count > 0 {
        return None;
    }
    notifications.iter().find(|n| n.contains(FAILSAFE_MARKER)).map(|n| {
        let detail = n.rsplit("returning partial document:").next().unwrap_or(n).trim();
        format!("the file is damaged or not a supported DWG version ({detail})")
    })
}

/// A parsed http(s) URL: only the parts the server needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpUrl {
    pub raw: String,
    /// Last path segment, percent-decoded; empty when the path ends in a slash.
    pub last_segment: String,
}

pub fn parse_http_url(raw: &str) -> Result<HttpUrl, ToolError> {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &trimmed[8..]
    } else if lower.starts_with("http://") {
        &trimmed[7..]
    } else {
        return Err(ToolError::InvalidArgument(format!(
            "url must start with http:// or https://, got {raw:?}"
        )));
    };
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    if host_end == 0 || rest[..host_end].contains(char::is_whitespace) {
        return Err(ToolError::InvalidArgument(format!("url has no host: {raw:?}")));
    }
    let after_host = &rest[host_end..];
    let path_end = after_host.find(['?', '#']).unwrap_or(after_host.len());
    let path = &after_host[..path_end];
    let last = path.rsplit('/').next().unwrap_or("");
    Ok(HttpUrl { raw: trimmed.to_string(), last_segment: percent_decode(last) })
}

pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// File name from a Content-Disposition header value, if it carries one.
pub fn filename_from_content_disposition(value: &str) -> Option<String> {
    let mut plain = None;
    for part in value.split(';').map(str::trim) {
        let lower = part.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("filename*=") {
            let raw = &part[part.len() - rest.len()..];
            let encoded = raw.rsplit("''").next().unwrap_or(raw);
            let name = percent_decode(encoded.trim_matches('"'));
            if !name.is_empty() {
                return Some(base_name(&name));
            }
        } else if let Some(rest) = lower.strip_prefix("filename=") {
            let raw = &part[part.len() - rest.len()..];
            let name = raw.trim_matches('"');
            if !name.is_empty() {
                plain = Some(base_name(name));
            }
        }
    }
    plain
}

fn base_name(name: &str) -> String {
    name.rsplit(['/', '\\']).next().unwrap_or(name).to_string()
}

/// The file name a download is known by: the header wins over the URL path.
pub fn download_file_name(content_disposition: Option<&str>, url: &HttpUrl) -> String {
    content_disposition
        .and_then(filename_from_content_disposition)
        .unwrap_or_else(|| url.last_segment.clone())
}

pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 15) as usize] as char);
    }
    s
}

/// Name of the result cache entry for one drawing content and one set of options.
pub fn result_key(sha256_hex: &str, opts: ParseOptions) -> String {
    format!("{sha256_hex}-s{}-t{}", opts.size, u8::from(opts.tiles))
}

/// The content hash a result key was built from.
pub fn result_key_sha(key: &str) -> &str {
    key.split('-').next().unwrap_or(key)
}

pub fn expires_at(created_unix: u64) -> u64 {
    created_unix + CACHE_TTL_SECS
}

pub fn is_expired(created_unix: u64, now_unix: u64) -> bool {
    now_unix >= expires_at(created_unix)
}

pub fn parse_cost(cached: bool) -> f64 {
    if cached {
        COST_CACHED_PARSE
    } else {
        COST_FRESH_PARSE
    }
}

/// Unix seconds as RFC 3339 UTC, for example 2026-11-24T10:00:00Z.
pub fn rfc3339(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Civil-from-days (Howard Hinnant), valid for the proleptic Gregorian calendar.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// One page of a listing.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub next_cursor: Option<String>,
}

fn resolve_limit(limit: Option<u32>, default: u32, max: u32) -> Result<usize, ToolError> {
    let limit = limit.unwrap_or(default);
    if !(1..=max).contains(&limit) {
        return Err(ToolError::InvalidArgument(format!(
            "limit must be between 1 and {max}, got {limit}"
        )));
    }
    Ok(limit as usize)
}

pub fn labels_limit(limit: Option<u32>) -> Result<usize, ToolError> {
    resolve_limit(limit, LABELS_LIMIT_DEFAULT, LABELS_LIMIT_MAX)
}

pub fn jobs_limit(limit: Option<u32>) -> Result<usize, ToolError> {
    resolve_limit(limit, JOBS_LIMIT_DEFAULT, JOBS_LIMIT_MAX)
}

/// A cursor is the offset of the next item, as decimal digits.
pub fn decode_cursor(cursor: Option<&str>) -> Result<usize, ToolError> {
    match cursor {
        None => Ok(0),
        Some(c) => c
            .parse::<usize>()
            .ok()
            .filter(|_| !c.is_empty() && c.len() <= 9 && c.bytes().all(|b| b.is_ascii_digit()))
            .ok_or_else(|| {
                ToolError::InvalidArgument(format!(
                    "cursor {c:?} is not a next_cursor returned by this tool"
                ))
            }),
    }
}

pub fn paginate<T: Clone>(items: &[T], offset: usize, limit: usize) -> Page<T> {
    let start = offset.min(items.len());
    let end = (start + limit).min(items.len());
    Page {
        items: items[start..end].to_vec(),
        total: items.len() as u32,
        next_cursor: (end < items.len()).then(|| end.to_string()),
    }
}

/// Labels that pass the optional page, layer and text filters, in page order.
pub fn filter_labels(
    labels: &[Label],
    page: Option<u32>,
    layer: Option<&str>,
    query: Option<&str>,
) -> Vec<Label> {
    let layer = layer.map(str::to_lowercase);
    let query = query.map(str::to_lowercase);
    labels
        .iter()
        .filter(|l| page.is_none_or(|p| l.page == p))
        .filter(|l| layer.as_deref().is_none_or(|n| l.layer.to_lowercase() == n))
        .filter(|l| query.as_deref().is_none_or(|q| l.text.to_lowercase().contains(q)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /* REQ-MCP-U01: timestamps are rendered as RFC 3339 UTC */
    #[test]
    fn rfc3339_formats_known_instants() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_795_514_400), "2026-11-24T10:00:00Z");
    }

    /* REQ-MCP-U02: a cache entry expires exactly seven days after it was created */
    #[test]
    fn expiry_is_seven_days() {
        assert!(!is_expired(1_000, 1_000 + CACHE_TTL_SECS - 1));
        assert!(is_expired(1_000, 1_000 + CACHE_TTL_SECS));
    }

    /* REQ-MCP-U03: a cached parse costs one tenth of a fresh one */
    #[test]
    fn cached_parse_is_ten_times_cheaper() {
        assert_eq!(parse_cost(false), 1.0);
        assert_eq!(parse_cost(true) * 10.0, parse_cost(false));
    }

    /* REQ-MCP-U04: the file name of a download comes from the header, then the URL path */
    #[test]
    fn download_name_prefers_content_disposition() {
        let url = parse_http_url("https://example.com/files/get?id=7").unwrap();
        assert_eq!(download_file_name(Some("attachment; filename=\"a b.dwg\""), &url), "a b.dwg");
        assert_eq!(
            download_file_name(Some("attachment; filename*=UTF-8''plan%20A.dwg"), &url),
            "plan A.dwg"
        );
        assert_eq!(download_file_name(None, &url), "get");
        let named = parse_http_url("http://h/a/site%20plan.DWG?x=1#f").unwrap();
        assert_eq!(download_file_name(None, &named), "site plan.DWG");
    }

    /* REQ-MCP-U05: only http and https URLs with a host are accepted */
    #[test]
    fn url_scheme_and_host_are_checked() {
        assert!(parse_http_url("ftp://h/a.dwg").is_err());
        assert!(parse_http_url("file:///a.dwg").is_err());
        assert!(parse_http_url("http:///a.dwg").is_err());
        assert!(parse_http_url("HTTPS://Example.com/a.dwg").is_ok());
    }

    /* REQ-MCP-U06: paging returns a cursor until the last page */
    #[test]
    fn paginate_walks_all_items() {
        let items: Vec<u32> = (0..5).collect();
        let first = paginate(&items, 0, 2);
        assert_eq!((first.items.clone(), first.total), (vec![0, 1], 5));
        let second = paginate(&items, decode_cursor(first.next_cursor.as_deref()).unwrap(), 2);
        let third = paginate(&items, decode_cursor(second.next_cursor.as_deref()).unwrap(), 2);
        assert_eq!(third.items, vec![4]);
        assert_eq!(third.next_cursor, None);
        assert!(decode_cursor(Some("-1")).is_err());
        assert!(decode_cursor(Some("abc")).is_err());
    }

    /* REQ-MCP-U07: the DWG signature is AC followed by four digits */
    #[test]
    fn dwg_magic_is_recognized() {
        assert!(has_dwg_magic(b"AC1032\0\0"));
        assert!(!has_dwg_magic(b"<html>"));
        assert!(!has_dwg_magic(b"AC10"));
    }

    /* REQ-MCP-U09: a failsafe-recovered empty document is reported as unreadable */
    #[test]
    fn failsafe_empty_document_is_unreadable() {
        let failed = vec![
            "[Warning] Reading DWG file version: AC1032 (AC24)".to_string(),
            "[Error] Failsafe: file header read failed, returning partial document: IO error: failed to fill whole buffer".to_string(),
        ];
        let reason = unreadable_reason(&failed, 0).unwrap();
        assert!(reason.contains("failed to fill whole buffer"));
        assert_eq!(unreadable_reason(&failed, 12), None);
        assert_eq!(unreadable_reason(&failed[..1], 0), None);
    }

    /* REQ-MCP-U08: a result key separates content, size and tiles */
    #[test]
    fn result_key_depends_on_options() {
        let a = result_key("ab12", ParseOptions { size: 3000, tiles: true });
        let b = result_key("ab12", ParseOptions { size: 3000, tiles: false });
        let c = result_key("ab12", ParseOptions { size: 2000, tiles: true });
        assert!(a != b && a != c && b != c);
        assert_eq!(result_key_sha(&a), "ab12");
    }
}
