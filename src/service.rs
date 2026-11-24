//! The eight tools as plain functions: validate, consult the cache, run the
//! renderer, record the job. The clock is passed in; MCP types stay in `server`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ab_glyph::FontVec;
use serde::Deserialize;

use crate::contract::{
    AttributeInfo, DeleteJobOutput, EntityCount, GetResultInput, GetResultOutput, ImageInfo,
    Index, Job, JobStatus, Label, LayoutInfo, ListJobsInput, ListJobsOutput, PageSummary,
    ParseFileInput, ParseOutput, ParseUrlInput, ResetCacheOutput, ResetFileCacheInput,
    ResetUrlCacheInput, ResultMode, Source, SourceKind, Summary, MAX_FILE_BYTES,
    MAX_INLINE_IMAGE_BYTES, SIZE_MAX, TILE_PX,
};
use crate::domain::{
    decode_cursor, expires_at, filter_labels, has_drawing_extension, is_drawing_content,
    is_text_dxf, jobs_limit, text_dxf_is_complete,
    labels_limit, paginate, parse_cost, parse_http_url, resolve_options, result_key, rfc3339,
    unreadable_reason, ParseOptions, ToolError, FORMAT_HEAD_BYTES,
};
use crate::fetch;
use crate::store::{sha256_bytes, sha256_file, DownloadMeta, JobRecord, ResultMeta, Store};
use dwg2png::render::RenderOpts;
use dwg2png::{process_file, FileReport, OutDirs};

/// The instant of a tool call, read once by the entrypoint.
#[derive(Debug, Clone, Copy)]
pub struct Now {
    pub unix: u64,
    pub nanos: u128,
}

pub struct Service {
    store: Store,
    font: FontVec,
    counter: AtomicU64,
}

/// A page PNG to return as image content next to the structured result.
pub struct InlineImage {
    pub png: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct DiskLabel {
    text: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    rot: f64,
    color: String,
    layer: String,
}

#[derive(Debug, Deserialize)]
struct DiskLabels {
    labels: Vec<DiskLabel>,
}

#[derive(Debug, Deserialize)]
struct DiskTile {
    file: String,
    col: u32,
    row: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    labels: Vec<DiskLabel>,
}

#[derive(Debug, Deserialize)]
struct DiskTiles {
    tiles: Vec<DiskTile>,
}

fn to_label(l: DiskLabel, page: u32) -> Label {
    Label { text: l.text, page, x: l.x, y: l.y, w: l.w, h: l.h, rot: l.rot, color: l.color, layer: l.layer }
}

fn read_disk<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ToolError> {
    let text = fs::read_to_string(path)
        .map_err(|e| ToolError::Internal(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| ToolError::Internal(format!("decode {}: {e}", path.display())))
}

fn page_stem(page: u32) -> String {
    format!("f00-p{page:02}")
}

/// A local drawing (DWG or DXF) that passed every check, with its content hash.
struct LocalDwg {
    path: PathBuf,
    file_name: String,
    sha256: String,
}

fn check_local_path(raw: &str) -> Result<PathBuf, ToolError> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(ToolError::InvalidArgument(format!(
            "path must be absolute, got {raw:?}"
        )));
    }
    let meta = fs::metadata(&path)
        .map_err(|_| ToolError::NotFound(format!("no file at {raw:?}")))?;
    if !meta.is_file() {
        return Err(ToolError::InvalidArgument(format!("{raw:?} is not a file")));
    }
    Ok(path)
}

fn check_local_dwg(raw: &str) -> Result<LocalDwg, ToolError> {
    let path = check_local_path(raw)?;
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
    if !has_drawing_extension(&file_name) {
        return Err(ToolError::UnsupportedFile(format!(
            "{file_name:?} is not a .dwg or .dxf file; only DWG and DXF drawings are supported"
        )));
    }
    let len = fs::metadata(&path)
        .map_err(|e| ToolError::Internal(format!("stat {}: {e}", path.display())))?
        .len();
    if len > MAX_FILE_BYTES {
        return Err(ToolError::TooLarge(format!(
            "{file_name:?} is {len} bytes; the limit is {MAX_FILE_BYTES} bytes"
        )));
    }
    let mut head = [0u8; FORMAT_HEAD_BYTES];
    let read = fs::File::open(&path)
        .and_then(|mut f| f.read(&mut head))
        .map_err(|e| ToolError::Internal(format!("read {}: {e}", path.display())))?;
    if !is_drawing_content(&head[..read]) {
        return Err(ToolError::UnsupportedFile(format!(
            "{file_name:?} does not start like a DWG or a DXF; it is not a drawing this server reads"
        )));
    }
    let sha256 = sha256_file(&path)?;
    Ok(LocalDwg { path, file_name, sha256 })
}

/// Why a file cannot be a whole drawing, judged before parsing: a text DXF that does
/// not end with EOF was cut short.
fn incomplete_reason(path: &Path) -> Result<Option<String>, ToolError> {
    use std::io::{Seek, SeekFrom};
    let io = |e: std::io::Error| ToolError::Internal(format!("read {}: {e}", path.display()));
    let mut file = fs::File::open(path).map_err(io)?;
    let mut head = [0u8; FORMAT_HEAD_BYTES];
    let read = file.read(&mut head).map_err(io)?;
    if !is_text_dxf(&head[..read]) {
        return Ok(None);
    }
    let len = file.metadata().map_err(io)?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(64))).map_err(io)?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail).map_err(io)?;
    Ok((!text_dxf_is_complete(&tail))
        .then(|| "the DXF is incomplete: it does not end with EOF, so it was cut short".to_string()))
}

fn summarize(report: &FileReport, file_name: &str, opts: ParseOptions) -> (Summary, Index) {
    let pages: Vec<PageSummary> = report
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| PageSummary {
            index: i as u32,
            name: p.name.clone(),
            width: p.width,
            height: p.height,
            label_count: p.labels.len() as u32,
            tile_rows: p.tiles.iter().map(|t| t.row + 1).max().unwrap_or(0),
            tile_cols: p.tiles.iter().map(|t| t.col + 1).max().unwrap_or(0),
        })
        .collect();
    let ix = report.index.as_ref();
    let layers = ix.map(|d| d.layers.clone()).unwrap_or_default();
    let blocks = ix.map(|d| d.blocks.clone()).unwrap_or_default();
    let version = ix.map(|d| d.version.clone()).unwrap_or_default();
    let entity_count = ix.map(|d| d.entity_count as u32).unwrap_or(0);
    let attributes: Vec<AttributeInfo> = ix
        .map(|d| {
            d.attributes
                .iter()
                .map(|a| AttributeInfo { tag: a.tag.clone(), value: a.value.clone() })
                .collect()
        })
        .unwrap_or_default();
    let summary = Summary {
        file_name: file_name.to_string(),
        version: version.clone(),
        page_count: pages.len() as u32,
        label_count: pages.iter().map(|p| p.label_count).sum(),
        pages,
        entity_count,
        layer_count: layers.len() as u32,
        layers: layers.clone(),
        block_count: blocks.len() as u32,
        attribute_count: attributes.len() as u32,
        size: opts.size,
        tiles: opts.tiles,
    };
    let index = Index {
        file_name: file_name.to_string(),
        version,
        entity_count,
        layers,
        blocks,
        layouts: ix
            .map(|d| {
                d.layouts
                    .iter()
                    .map(|l| LayoutInfo {
                        name: l.name.clone(),
                        paper_w: l.paper_w,
                        paper_h: l.paper_h,
                        entity_count: l.entity_count as u32,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        histogram: ix
            .map(|d| {
                d.histogram
                    .iter()
                    .map(|(k, v)| EntityCount { kind: k.clone(), count: *v as u32 })
                    .collect()
            })
            .unwrap_or_default(),
        attributes,
        text_samples: ix.map(|d| d.text_samples.clone()).unwrap_or_default(),
    };
    (summary, index)
}

fn to_job(r: &JobRecord) -> Job {
    Job {
        job_id: r.job_id.clone(),
        status: r.status,
        source: r.source.clone(),
        file_name: r.file_name.clone(),
        sha256: r.sha256.clone(),
        cached: r.cached,
        cost_units: r.cost_units,
        created_at: rfc3339(r.created_unix),
        expires_at: rfc3339(expires_at(r.result_created_unix)),
        parse_ms: r.parse_ms,
        render_ms: r.render_ms,
        error: r.error.clone(),
    }
}

impl Service {
    pub fn new(cache_dir: PathBuf, font: FontVec) -> Result<Service, ToolError> {
        Ok(Service { store: Store::open(cache_dir)?, font, counter: AtomicU64::new(0) })
    }

    fn new_job_id(&self, now: Now) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let seed = format!("{}-{}-{n}", now.nanos, std::process::id());
        format!("job_{}", &sha256_bytes(seed.as_bytes())[..16])
    }

    /// Render one drawing into a staging directory and publish it under `key`.
    fn render(
        &self,
        dwg: &Path,
        file_name: &str,
        key: &str,
        job_id: &str,
        opts: ParseOptions,
        now: Now,
    ) -> Result<Result<ResultMeta, String>, ToolError> {
        if let Some(reason) = incomplete_reason(dwg)? {
            return Ok(Err(reason));
        }
        let staging = self.store.staging_dir(key, job_id)?;
        let out = staging.join("out");
        let (img, labels, tiles) = (out.join("img"), out.join("labels"), out.join("tiles"));
        for dir in [&img, &labels, &tiles] {
            fs::create_dir_all(dir)
                .map_err(|e| ToolError::Internal(format!("create {}: {e}", dir.display())))?;
        }
        let dirs = OutDirs {
            img_dir: &img,
            img_rel: "img",
            labels_dir: &labels,
            tiles_dir: &tiles,
            tiles_rel: "tiles",
        };
        let render_opts = RenderOpts {
            target_long: opts.size as f32,
            max_side: SIZE_MAX,
            tile_px: opts.tiles.then_some(TILE_PX),
        };
        let report = process_file(dwg, &dirs, &self.font, 0, render_opts);
        if !report.ok {
            self.store.discard_staging(&staging)?;
            return Ok(Err(report.error.unwrap_or_else(|| "the drawing has no pages".into())));
        }
        let unreadable = report
            .index
            .as_ref()
            .and_then(|ix| unreadable_reason(&ix.notifications, ix.entity_count));
        if let Some(reason) = unreadable {
            self.store.discard_staging(&staging)?;
            return Ok(Err(reason));
        }
        let (summary, index) = summarize(&report, file_name, opts);
        let meta = ResultMeta {
            created_unix: now.unix,
            parse_ms: report.parse_ms as u64,
            render_ms: report.render_ms as u64,
            summary,
            index,
        };
        self.store.publish_result(key, &staging, &meta)?;
        Ok(Ok(meta))
    }

    /// Answer from the result cache, or parse and cache. Always records a job.
    fn parse_content(
        &self,
        source: Source,
        dwg: &Path,
        file_name: &str,
        sha256: &str,
        opts: ParseOptions,
        now: Now,
    ) -> Result<ParseOutput, ToolError> {
        let key = result_key(sha256, opts);
        let job_id = self.new_job_id(now);
        let cached_meta = self.store.get_result(&key)?;
        let cached = cached_meta.is_some();
        let outcome = match cached_meta {
            Some(meta) => Ok(meta),
            None => self.render(dwg, file_name, &key, &job_id, opts, now)?,
        };
        let mut record = JobRecord {
            job_id,
            status: JobStatus::Completed,
            source,
            file_name: file_name.to_string(),
            sha256: sha256.to_string(),
            result_key: Some(key),
            cached,
            cost_units: parse_cost(cached),
            created_unix: now.unix,
            created_ns: now.nanos as u64,
            result_created_unix: now.unix,
            parse_ms: 0,
            render_ms: 0,
            error: None,
        };
        match outcome {
            Ok(meta) => {
                record.result_created_unix = meta.created_unix;
                record.parse_ms = meta.parse_ms;
                record.render_ms = meta.render_ms;
                self.store.put_job(&record)?;
                // A cached result keeps the name it was first parsed under.
                let mut summary = meta.summary;
                summary.file_name = file_name.to_string();
                Ok(ParseOutput { job: to_job(&record), download_cached: None, summary })
            }
            Err(reason) => {
                record.status = JobStatus::Failed;
                record.result_key = None;
                record.cost_units = 0.0;
                record.error = Some(reason.clone());
                self.store.put_job(&record)?;
                Err(ToolError::ParseFailed(format!(
                    "{file_name:?} could not be parsed: {reason}. Recorded as job {} (not billed).",
                    record.job_id
                )))
            }
        }
    }

    pub fn parse_file(&self, input: ParseFileInput, now: Now) -> Result<ParseOutput, ToolError> {
        self.store.purge_expired(now.unix)?;
        let opts = resolve_options(input.size, input.tiles)?;
        let dwg = check_local_dwg(&input.path)?;
        let source = Source { kind: SourceKind::File, value: input.path };
        self.parse_content(source, &dwg.path, &dwg.file_name, &dwg.sha256, opts, now)
    }

    pub fn parse_url(&self, input: ParseUrlInput, now: Now) -> Result<ParseOutput, ToolError> {
        self.store.purge_expired(now.unix)?;
        let opts = resolve_options(input.size, input.tiles)?;
        let url = parse_http_url(&input.url)?;
        let (meta, download_cached) = match self.store.get_download(&url.raw)? {
            Some(meta) => (meta, true),
            None => {
                let got = fetch::download(&url)?;
                let head = &got.bytes[..got.bytes.len().min(FORMAT_HEAD_BYTES)];
                if !is_drawing_content(head) {
                    return Err(ToolError::UnsupportedFile(format!(
                        "{} does not start like a DWG or a DXF; it is not a drawing this server reads",
                        url.raw
                    )));
                }
                let meta = DownloadMeta {
                    url: url.raw.clone(),
                    file_name: got.file_name,
                    sha256: sha256_bytes(&got.bytes),
                    bytes: got.bytes.len() as u64,
                    fetched_unix: now.unix,
                };
                self.store.put_download(&meta, &got.bytes)?;
                (meta, false)
            }
        };
        let source = Source { kind: SourceKind::Url, value: url.raw.clone() };
        let file = self.store.download_file(&url.raw);
        let mut out = self.parse_content(source, &file, &meta.file_name, &meta.sha256, opts, now)?;
        out.download_cached = Some(download_cached);
        Ok(out)
    }

    fn require_job(&self, job_id: &str) -> Result<JobRecord, ToolError> {
        self.store.get_job(job_id)?.ok_or_else(|| {
            ToolError::NotFound(format!(
                "no job {job_id:?}; it may have expired or been deleted. list_jobs shows current jobs."
            ))
        })
    }

    pub fn get_job(&self, job_id: &str, now: Now) -> Result<Job, ToolError> {
        self.store.purge_expired(now.unix)?;
        self.require_job(job_id).map(|r| to_job(&r))
    }

    pub fn list_jobs(&self, input: ListJobsInput, now: Now) -> Result<ListJobsOutput, ToolError> {
        self.store.purge_expired(now.unix)?;
        let limit = jobs_limit(input.limit)?;
        let offset = decode_cursor(input.cursor.as_deref())?;
        let jobs: Vec<Job> = self.store.list_jobs()?.iter().map(to_job).collect();
        let total_cost_units = (jobs.iter().map(|j| j.cost_units).sum::<f64>() * 10.0).round() / 10.0;
        let page = paginate(&jobs, offset, limit);
        Ok(ListJobsOutput {
            jobs: page.items,
            total: page.total,
            total_cost_units,
            next_cursor: page.next_cursor,
        })
    }

    pub fn delete_job(&self, job_id: &str, now: Now) -> Result<DeleteJobOutput, ToolError> {
        self.store.purge_expired(now.unix)?;
        let job = self.require_job(job_id)?;
        self.store.remove_job(&job.job_id)?;
        let mut result_removed = false;
        if let Some(key) = &job.result_key {
            let shared = self.store.list_jobs()?.iter().any(|j| j.result_key.as_ref() == Some(key));
            if !shared {
                result_removed = self.store.remove_result(key)?;
            }
        }
        Ok(DeleteJobOutput { job_id: job.job_id, deleted: true, result_removed })
    }

    pub fn reset_file_cache(
        &self,
        input: ResetFileCacheInput,
        now: Now,
    ) -> Result<ResetCacheOutput, ToolError> {
        self.store.purge_expired(now.unix)?;
        let path = check_local_path(&input.path)?;
        let sha256 = sha256_file(&path)?;
        let (results_removed, jobs_removed) = self.store.remove_results_of_content(&sha256)?;
        Ok(ResetCacheOutput { downloads_removed: 0, results_removed, jobs_removed })
    }

    pub fn reset_url_cache(
        &self,
        input: ResetUrlCacheInput,
        now: Now,
    ) -> Result<ResetCacheOutput, ToolError> {
        self.store.purge_expired(now.unix)?;
        let url = parse_http_url(&input.url)?;
        let Some(meta) = self.store.get_download(&url.raw)? else {
            return Ok(ResetCacheOutput { downloads_removed: 0, results_removed: 0, jobs_removed: 0 });
        };
        let (results_removed, jobs_removed) = self.store.remove_results_of_content(&meta.sha256)?;
        let downloads_removed = u32::from(self.store.remove_download(&url.raw)?);
        Ok(ResetCacheOutput { downloads_removed, results_removed, jobs_removed })
    }

    /// The folder holding the files of a completed job: `img/`, `labels/`, `tiles/`
    /// with the names the renderer writes (`f00-pNN...`). For hosts that publish the
    /// files themselves, such as the Apify Actor.
    pub fn result_files_dir(&self, job_id: &str) -> Result<PathBuf, ToolError> {
        let job = self.require_job(job_id)?;
        let key = job.result_key.ok_or_else(|| {
            ToolError::ParseFailed(format!("job {} failed and has no result", job.job_id))
        })?;
        Ok(self.store.result_out_dir(&key))
    }

    fn page_labels(&self, out: &Path, page: u32) -> Result<Vec<Label>, ToolError> {
        let disk: DiskLabels = read_disk(&out.join("labels").join(format!("{}.json", page_stem(page))))?;
        Ok(disk.labels.into_iter().map(|l| to_label(l, page)).collect())
    }

    fn image_info(
        path: PathBuf,
        page: u32,
        tile: Option<&DiskTile>,
        size: (u32, u32),
    ) -> Result<(ImageInfo, Option<InlineImage>), ToolError> {
        let png = fs::read(&path)
            .map_err(|e| ToolError::Internal(format!("read {}: {e}", path.display())))?;
        let bytes = png.len() as u64;
        let inline = bytes <= MAX_INLINE_IMAGE_BYTES;
        let info = ImageInfo {
            page,
            row: tile.map(|t| t.row),
            col: tile.map(|t| t.col),
            x: tile.map_or(0, |t| t.x),
            y: tile.map_or(0, |t| t.y),
            width: size.0,
            height: size.1,
            mime_type: "image/png".into(),
            path: path.display().to_string(),
            bytes,
            inline,
        };
        Ok((info, inline.then_some(InlineImage { png })))
    }

    pub fn get_result(
        &self,
        input: GetResultInput,
        now: Now,
    ) -> Result<(GetResultOutput, Option<InlineImage>), ToolError> {
        self.store.purge_expired(now.unix)?;
        let job = self.require_job(&input.job_id)?;
        let Some(key) = job.result_key.as_deref() else {
            return Err(ToolError::ParseFailed(format!(
                "job {} failed and has no result: {}",
                job.job_id,
                job.error.as_deref().unwrap_or("unknown error")
            )));
        };
        let meta = self.store.get_result(key)?.ok_or_else(|| {
            ToolError::NotFound(format!(
                "the result of job {} is no longer cached; parse the drawing again",
                job.job_id
            ))
        })?;
        let mut summary = meta.summary;
        summary.file_name = job.file_name.clone();
        let mode = input.mode.unwrap_or(ResultMode::Summary);
        let out_dir = self.store.result_out_dir(key);
        let mut out = GetResultOutput {
            job_id: job.job_id.clone(),
            mode,
            summary: None,
            index: None,
            labels: None,
            total: None,
            next_cursor: None,
            image: None,
        };
        let check_page = |page: Option<u32>, required: bool| -> Result<Option<u32>, ToolError> {
            match page {
                None if required => Err(ToolError::InvalidArgument(format!(
                    "page is required for this mode; the drawing has pages 0 to {}",
                    summary.page_count.saturating_sub(1)
                ))),
                Some(p) if p >= summary.page_count => Err(ToolError::InvalidArgument(format!(
                    "page {p} does not exist; the drawing has pages 0 to {}",
                    summary.page_count.saturating_sub(1)
                ))),
                other => Ok(other),
            }
        };
        match mode {
            ResultMode::Summary => {
                out.summary = Some(summary);
                Ok((out, None))
            }
            ResultMode::Index => {
                let mut index = meta.index;
                index.file_name = job.file_name;
                out.index = Some(index);
                Ok((out, None))
            }
            ResultMode::Labels => {
                let limit = labels_limit(input.limit)?;
                let offset = decode_cursor(input.cursor.as_deref())?;
                let page = check_page(input.page, false)?;
                let mut all = Vec::new();
                for p in 0..summary.page_count {
                    if page.is_none_or(|only| only == p) {
                        all.extend(self.page_labels(&out_dir, p)?);
                    }
                }
                let matching = filter_labels(&all, None, input.layer.as_deref(), input.query.as_deref());
                let listing = paginate(&matching, offset, limit);
                out.labels = Some(listing.items);
                out.total = Some(listing.total);
                out.next_cursor = listing.next_cursor;
                Ok((out, None))
            }
            ResultMode::PageImage => {
                let page = check_page(input.page, true)?.unwrap_or(0);
                let p = &summary.pages[page as usize];
                let path = out_dir.join("img").join(format!("{}.png", page_stem(page)));
                let (info, image) = Self::image_info(path, page, None, (p.width, p.height))?;
                out.image = Some(info);
                Ok((out, image))
            }
            ResultMode::TileImage => {
                let page = check_page(input.page, true)?.unwrap_or(0);
                if !summary.tiles {
                    return Err(ToolError::InvalidArgument(
                        "this drawing was parsed with tiles=false; parse it again with tiles=true, or use mode page_image".into(),
                    ));
                }
                let p = &summary.pages[page as usize];
                if p.tile_rows == 0 {
                    return Err(ToolError::InvalidArgument(format!(
                        "page {page} is {}x{} px, not larger than a {TILE_PX} px tile, so it has no tiles; use mode page_image, or parse with a larger size",
                        p.width, p.height
                    )));
                }
                let (Some(row), Some(col)) = (input.row, input.col) else {
                    return Err(ToolError::InvalidArgument(format!(
                        "row and col are required for mode tile_image; page {page} has {} rows and {} columns",
                        p.tile_rows, p.tile_cols
                    )));
                };
                if row >= p.tile_rows || col >= p.tile_cols {
                    return Err(ToolError::InvalidArgument(format!(
                        "tile row {row}, col {col} does not exist; page {page} has {} rows and {} columns",
                        p.tile_rows, p.tile_cols
                    )));
                }
                let tile_dir = out_dir.join("tiles").join(page_stem(page));
                let manifest: DiskTiles = read_disk(&tile_dir.join("tiles.json"))?;
                let tile = manifest
                    .tiles
                    .into_iter()
                    .find(|t| t.row == row && t.col == col)
                    .ok_or_else(|| {
                        ToolError::Internal(format!("tile r{row}c{col} is missing from the manifest"))
                    })?;
                let path = tile_dir.join(&tile.file);
                let (info, image) =
                    Self::image_info(path, page, Some(&tile), (tile.width, tile.height))?;
                out.image = Some(info);
                out.labels = Some(tile.labels.into_iter().map(|l| to_label(l, page)).collect());
                Ok((out, image))
            }
        }
    }
}
