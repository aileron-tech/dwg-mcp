//! Wire contract of the MCP server: tool inputs, tool outputs, limits.
//!
//! These types are the public contract. Field names and enum values are part
//! of it; change them only with a migration plan.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Downloads and results are kept for seven days.
pub const CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;
/// Cost of a parse that ran the renderer.
pub const COST_FRESH_PARSE: f64 = 1.0;
/// Cost of a parse answered from the result cache: ten times cheaper.
pub const COST_CACHED_PARSE: f64 = 0.1;
/// Largest DWG accepted, from disk or from a URL.
pub const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
/// Largest PNG returned inline as image content; larger ones return a path only.
pub const MAX_INLINE_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

pub const SIZE_DEFAULT: u32 = 3000;
pub const SIZE_MIN: u32 = 512;
pub const SIZE_MAX: u32 = 12000;
pub const TILE_PX: u32 = 1500;
pub const LABELS_LIMIT_DEFAULT: u32 = 100;
pub const LABELS_LIMIT_MAX: u32 = 500;
pub const JOBS_LIMIT_DEFAULT: u32 = 50;
pub const JOBS_LIMIT_MAX: u32 = 100;

// ---------------------------------------------------------------- inputs --

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParseFileInput {
    /// Absolute path of a .dwg file on the machine that runs this server. At most 100 MB.
    pub path: String,
    /// Long edge of each page image in pixels, 512 to 12000. Default 3000. A different
    /// size is a different cache entry.
    #[serde(default)]
    pub size: Option<u32>,
    /// Also cut each page into readable 1500 px tiles for vision models. Default true. A
    /// different value is a different cache entry.
    #[serde(default)]
    pub tiles: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParseUrlInput {
    /// http or https URL of a .dwg file. The file name comes from Content-Disposition or
    /// the URL path and must end in .dwg. At most 100 MB.
    pub url: String,
    /// Long edge of each page image in pixels, 512 to 12000. Default 3000. A different
    /// size is a different cache entry.
    #[serde(default)]
    pub size: Option<u32>,
    /// Also cut each page into readable 1500 px tiles for vision models. Default true. A
    /// different value is a different cache entry.
    #[serde(default)]
    pub tiles: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResultMode {
    /// Pages, sizes, label counts, layers: the overview to read first.
    Summary,
    /// Full metadata index: layers, blocks, layouts, entity histogram, attributes.
    Index,
    /// Labels as exact text with pixel boxes, filtered and paged.
    Labels,
    /// One whole page as a PNG.
    PageImage,
    /// One readable tile of a page as a PNG, with the labels inside it.
    TileImage,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetResultInput {
    /// The job_id returned by parse_file or parse_url.
    pub job_id: String,
    /// Which part of the result to return. Default summary.
    #[serde(default)]
    pub mode: Option<ResultMode>,
    /// Zero-based page index (0 is Model space). Required for page_image and tile_image;
    /// optional filter for labels.
    #[serde(default)]
    pub page: Option<u32>,
    /// Labels only: keep labels on this layer (exact name, case-insensitive).
    #[serde(default)]
    pub layer: Option<String>,
    /// Labels only: keep labels whose text contains this string (case-insensitive).
    #[serde(default)]
    pub query: Option<String>,
    /// tile_image only: zero-based tile row, from the tile grid in the summary.
    #[serde(default)]
    pub row: Option<u32>,
    /// tile_image only: zero-based tile column, from the tile grid in the summary.
    #[serde(default)]
    pub col: Option<u32>,
    /// Labels only: page size, 1 to 500. Default 100.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Labels only: the next_cursor of the previous call, to continue the listing.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JobIdInput {
    /// The job_id returned by parse_file or parse_url, or listed by list_jobs.
    pub job_id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListJobsInput {
    /// Page size, 1 to 100. Default 50.
    #[serde(default)]
    pub limit: Option<u32>,
    /// The next_cursor of the previous call, to continue the listing.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResetFileCacheInput {
    /// Absolute path of the .dwg file whose cached results should be dropped. The file
    /// is matched by content, so it must still exist.
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResetUrlCacheInput {
    /// The exact URL that was given to parse_url.
    pub url: String,
}

// --------------------------------------------------------------- outputs --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    File,
    Url,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    pub kind: SourceKind,
    /// The path or URL that was parsed.
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PageSummary {
    /// Zero-based page index; 0 is Model space, the rest are paper sheets.
    pub index: u32,
    pub name: String,
    /// Page image width in pixels.
    pub width: u32,
    /// Page image height in pixels.
    pub height: u32,
    pub label_count: u32,
    /// Tile grid size; 0 when the drawing was parsed without tiles or the page is not
    /// larger than one 1500 px tile (read such a page with mode page_image).
    pub tile_rows: u32,
    pub tile_cols: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Summary {
    pub file_name: String,
    /// DWG version code, for example AC1032.
    pub version: String,
    pub page_count: u32,
    pub pages: Vec<PageSummary>,
    /// Labels over all pages.
    pub label_count: u32,
    pub entity_count: u32,
    pub layer_count: u32,
    pub layers: Vec<String>,
    pub block_count: u32,
    pub attribute_count: u32,
    /// Long edge in pixels the pages were rendered at.
    pub size: u32,
    pub tiles: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Job {
    pub job_id: String,
    pub status: JobStatus,
    pub source: Source,
    pub file_name: String,
    /// SHA-256 of the drawing's bytes; empty when the file could not be read.
    pub sha256: String,
    /// True when the result came from the cache instead of a new parse.
    pub cached: bool,
    /// 1 for a new parse, 0.1 for a cache hit, 0 for a failed parse.
    pub cost_units: f64,
    /// When the job was created, RFC 3339 UTC.
    pub created_at: String,
    /// When the result leaves the cache, RFC 3339 UTC. Seven days after the parse that
    /// produced it; a cache hit does not extend it.
    pub expires_at: String,
    pub parse_ms: u64,
    pub render_ms: u64,
    /// Why the parse failed; absent for a completed job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParseOutput {
    pub job: Job,
    /// parse_url only: true when the file came from the download cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_cached: Option<bool>,
    pub summary: Summary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Label {
    /// Exact text from the drawing (no OCR).
    pub text: String,
    /// Zero-based page index the label is on.
    pub page: u32,
    /// Left end of the text baseline, in pixels of the page image (of the tile image
    /// for mode tile_image).
    pub x: f64,
    /// Baseline height, in pixels from the top of the same image.
    pub y: f64,
    /// Text width in pixels.
    pub w: f64,
    /// Text height in pixels.
    pub h: f64,
    /// Clockwise rotation on screen, degrees.
    pub rot: f64,
    /// Text color as #rrggbb.
    pub color: String,
    pub layer: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LayoutInfo {
    pub name: String,
    pub paper_w: f64,
    pub paper_h: f64,
    pub entity_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AttributeInfo {
    pub tag: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EntityCount {
    /// Entity type, for example Line, Text, Insert.
    pub kind: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Index {
    pub file_name: String,
    pub version: String,
    pub entity_count: u32,
    pub layers: Vec<String>,
    pub blocks: Vec<String>,
    pub layouts: Vec<LayoutInfo>,
    /// Entity counts by type, sorted by type name.
    pub histogram: Vec<EntityCount>,
    /// Title-block and block attribute values (at most 60).
    pub attributes: Vec<AttributeInfo>,
    /// A sample of text strings (at most 60); use mode labels for all of them.
    pub text_samples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImageInfo {
    pub page: u32,
    /// Tile row; present for tile_image only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<u32>,
    /// Tile column; present for tile_image only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
    /// Offset of the tile's left edge in page-image pixels; 0 for a page image.
    pub x: u32,
    /// Offset of the tile's top edge in page-image pixels; 0 for a page image.
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Always image/png.
    pub mime_type: String,
    /// Path of the PNG in the server's cache folder.
    pub path: String,
    pub bytes: u64,
    /// True when the PNG is also returned as image content; false above 5 MB.
    pub inline: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetResultOutput {
    pub job_id: String,
    pub mode: ResultMode,
    /// Present for mode summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<Summary>,
    /// Present for mode index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<Index>,
    /// Present for mode labels (one page of the listing, positions in page-image
    /// pixels) and tile_image (labels inside the tile, positions in tile-image pixels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<Label>>,
    /// Mode labels: number of labels that match the filters, over all pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    /// Mode labels: pass as cursor to get the next page; absent on the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Present for modes page_image and tile_image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListJobsOutput {
    /// Jobs, newest first.
    pub jobs: Vec<Job>,
    /// Number of jobs in the cache.
    pub total: u32,
    /// Sum of cost_units over all jobs in the cache.
    pub total_cost_units: f64,
    /// Pass as cursor to get the next page; absent on the last page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeleteJobOutput {
    pub job_id: String,
    pub deleted: bool,
    /// True when the cached result was removed too; false when another job still uses it.
    pub result_removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResetCacheOutput {
    /// Cached downloads removed (reset_url_cache only; always 0 for reset_file_cache).
    pub downloads_removed: u32,
    /// Cached parse results removed.
    pub results_removed: u32,
    /// Jobs removed because their result was removed.
    pub jobs_removed: u32,
}
