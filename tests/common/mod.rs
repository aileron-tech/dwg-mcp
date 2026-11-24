//! End-to-end harness: spawn the real `dwg-mcp` binary and drive it with
//! the official Rust MCP client over stdio.
#![allow(dead_code)]

pub mod fixtures;
pub mod http;

use std::path::{Path, PathBuf};

use base64::Engine;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt};
use serde_json::{json, Value};
use tempfile::TempDir;

const FONT_CANDIDATES: [&str; 3] = [
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
];

fn font_path() -> String {
    if let Ok(font) = std::env::var("DWG2PNG_FONT") {
        return font;
    }
    FONT_CANDIDATES
        .iter()
        .find(|p| Path::new(p).is_file())
        .map(|p| p.to_string())
        .expect("no TrueType font found; set DWG2PNG_FONT")
}

pub fn load_test_font() -> ab_glyph::FontVec {
    dwg2png::text::load_font(&font_path()).expect("load test font")
}

/// What one tool call returned, in the shapes the tests assert on.
#[derive(Debug)]
pub struct Reply {
    pub is_error: bool,
    /// `structuredContent` of the result; `Value::Null` when absent.
    pub data: Value,
    /// All text content blocks, joined.
    pub text: String,
    /// Decoded image content blocks: (mime type, bytes).
    pub images: Vec<(String, Vec<u8>)>,
}

impl Reply {
    fn from(result: CallToolResult) -> Reply {
        let mut text = String::new();
        let mut images = Vec::new();
        for block in &result.content {
            if let Some(t) = block.as_text() {
                text.push_str(&t.text);
            }
            if let Some(i) = block.as_image() {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&i.data)
                    .expect("image content is base64");
                images.push((i.mime_type.clone(), bytes));
            }
        }
        Reply {
            is_error: result.is_error.unwrap_or(false),
            data: result.structured_content.unwrap_or(Value::Null),
            text,
            images,
        }
    }

    /// The structured result of a call that must have succeeded.
    pub fn ok(self) -> Value {
        assert!(!self.is_error, "tool call failed: {}", self.text);
        assert!(!self.data.is_null(), "tool call returned no structured content");
        self.data
    }

    /// The error code of a call that must have failed ("code: message").
    pub fn err_code(&self) -> &str {
        assert!(self.is_error, "tool call succeeded: {}", self.text);
        self.text.split(':').next().unwrap_or("")
    }
}

pub struct Harness {
    pub client: RunningService<RoleClient, ()>,
    cache: TempDir,
    files: TempDir,
}

async fn spawn(cache_dir: &Path) -> RunningService<RoleClient, ()> {
    let command = tokio::process::Command::new(env!("CARGO_BIN_EXE_dwg-mcp")).configure(|cmd| {
        cmd.env("DWG_MCP_CACHE_DIR", cache_dir).env("DWG2PNG_FONT", font_path());
    });
    let transport = TokioChildProcess::new(command).expect("spawn dwg-mcp");
    ().serve(transport).await.expect("MCP handshake")
}

impl Harness {
    /// A server with an empty cache of its own.
    pub async fn start() -> Harness {
        let cache = TempDir::new().expect("cache dir");
        let files = TempDir::new().expect("files dir");
        let client = spawn(cache.path()).await;
        Harness { client, cache, files }
    }

    /// Stop the server and start a new one on the same cache folder.
    pub async fn restart(self) -> Harness {
        let Harness { client, cache, files } = self;
        client.cancel().await.expect("stop server");
        let client = spawn(cache.path()).await;
        Harness { client, cache, files }
    }

    pub async fn stop(self) {
        self.client.cancel().await.expect("stop server");
    }

    pub fn cache_dir(&self) -> &Path {
        self.cache.path()
    }

    /// Folder for the drawings a test writes.
    pub fn files_dir(&self) -> &Path {
        self.files.path()
    }

    pub fn plan(&self) -> PathBuf {
        fixtures::write_dwg(&fixtures::plan_document(), self.files_dir(), "plan.dwg")
    }

    pub fn blank(&self) -> PathBuf {
        fixtures::write_dwg(&fixtures::blank_document(), self.files_dir(), "blank.dwg")
    }

    pub fn dense(&self, n: usize) -> PathBuf {
        fixtures::write_dwg(&fixtures::dense_document(n), self.files_dir(), "dense.dwg")
    }

    pub async fn call(&self, tool: &'static str, args: Value) -> Reply {
        let arguments = args.as_object().cloned().expect("tool arguments are an object");
        let params = CallToolRequestParams::new(tool).with_arguments(arguments);
        Reply::from(self.client.call_tool(params).await.expect("tools/call transport"))
    }

    /// The raw outcome of tools/call, for protocol-level errors.
    pub async fn call_raw(&self, tool: &'static str, args: Value) -> Result<Reply, String> {
        let arguments = args.as_object().cloned().expect("tool arguments are an object");
        let params = CallToolRequestParams::new(tool).with_arguments(arguments);
        self.client.call_tool(params).await.map(Reply::from).map_err(|e| e.to_string())
    }

    pub async fn parse_file(&self, path: &Path) -> Value {
        self.call("parse_file", json!({ "path": path })).await.ok()
    }

    /// Parse with a small page size, to keep image tests fast.
    pub async fn parse_small(&self, path: &Path) -> Value {
        self.call("parse_file", json!({ "path": path, "size": 1024 })).await.ok()
    }

    pub async fn job_ids(&self) -> Vec<String> {
        let listing = self.call("list_jobs", json!({})).await.ok();
        listing["jobs"]
            .as_array()
            .expect("jobs array")
            .iter()
            .map(|j| j["job_id"].as_str().expect("job_id").to_string())
            .collect()
    }

    /// The tool definitions exactly as a client receives them from tools/list.
    pub async fn tools(&self) -> Vec<Value> {
        let tools = self.client.list_all_tools().await.expect("tools/list");
        tools.iter().map(|t| serde_json::to_value(t).expect("tool as JSON")).collect()
    }

    pub async fn tool(&self, name: &str) -> Value {
        self.tools()
            .await
            .into_iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("tool {name} is not listed"))
    }

    /// Make every cache entry look `days` days older by rewriting its timestamps on
    /// disk: the only way to cross the seven-day line in a test.
    pub fn age_cache(&self, days: u64) {
        let shift = days * 24 * 60 * 60;
        for file in json_files(self.cache_dir()) {
            let text = std::fs::read_to_string(&file).expect("read cache record");
            let mut value: Value = serde_json::from_str(&text).expect("cache record is JSON");
            let mut changed = false;
            for key in ["created_unix", "result_created_unix", "fetched_unix"] {
                if let Some(t) = value.get(key).and_then(Value::as_u64) {
                    value[key] = json!(t - shift);
                    changed = true;
                }
            }
            if changed {
                std::fs::write(&file, value.to_string()).expect("write cache record");
            }
        }
    }
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read cache dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Only the record files: jobs/*.json and */meta.json, not renderer output.
            if path.file_name().is_some_and(|n| n == "out") {
                continue;
            }
            found.extend(json_files(&path));
        } else if path.extension().is_some_and(|e| e == "json") {
            found.push(path);
        }
    }
    found
}

/// Width and height of a PNG, read from its IHDR chunk.
pub fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    let be = |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    (be(16), be(20))
}

pub fn strs(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|v| v.as_str().expect("string").to_string())
        .collect()
}

pub fn label_texts(result: &Value) -> Vec<String> {
    result["labels"]
        .as_array()
        .expect("labels array")
        .iter()
        .map(|l| l["text"].as_str().expect("label text").to_string())
        .collect()
}

pub fn job_id(parse_output: &Value) -> String {
    parse_output["job"]["job_id"].as_str().expect("job.job_id").to_string()
}

/// Unix seconds of an RFC 3339 UTC timestamp such as 2026-11-24T10:00:00Z.
pub fn unix_of(timestamp: &str) -> i64 {
    let n = |range: std::ops::Range<usize>| -> i64 {
        timestamp[range].parse().unwrap_or_else(|_| panic!("bad timestamp {timestamp}"))
    };
    assert!(timestamp.len() == 20 && timestamp.ends_with('Z'), "bad timestamp {timestamp}");
    let (year, month, day) = (n(0..4), n(5..7), n(8..10));
    // Days from civil (Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + n(11..13) * 3600 + n(14..16) * 60 + n(17..19)
}

pub const TOOL_NAMES: [&str; 8] = [
    "parse_file",
    "parse_url",
    "get_result",
    "get_job",
    "list_jobs",
    "delete_job",
    "reset_file_cache",
    "reset_url_cache",
];
