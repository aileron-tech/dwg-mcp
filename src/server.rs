//! MCP entrypoint: tool definitions, the clock, and the stdio serve loop.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use rmcp::handler::server::common::schema_for_output;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler, ServiceExt};
use serde::Serialize;

use crate::contract::{
    DeleteJobOutput, GetResultInput, GetResultOutput, Job, JobIdInput, ListJobsInput,
    ListJobsOutput, ParseFileInput, ParseOutput, ParseUrlInput, ResetCacheOutput,
    ResetFileCacheInput, ResetUrlCacheInput,
};
use crate::domain::ToolError;
use crate::service::{Now, Service};

const INSTRUCTIONS: &str = "dwg-mcp turns AutoCAD DWG and DXF drawings into data an AI application can \
use: page images, readable tiles, every text label with its pixel position, and a metadata \
index. Start with parse_file (local path) or parse_url (http/https link); both return a job_id \
and a summary. Then call get_result with that job_id: mode summary first, then labels, index, \
page_image or tile_image for the part you need. Results and downloads are cached for 7 days; a \
repeated parse of the same drawing is answered from the cache at one tenth of the cost.";

#[derive(Clone)]
pub struct DwgServer {
    service: Arc<Service>,
    /// One tool call at a time: the cache directory is shared state.
    gate: Arc<Mutex<()>>,
    tool_router: ToolRouter<Self>,
}

fn now() -> Now {
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    Now { unix: since_epoch.as_secs(), nanos: since_epoch.as_nanos() }
}

fn tool_error(e: &ToolError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(e.to_string())])
}

fn structured<T: Serialize>(value: &T) -> CallToolResult {
    match serde_json::to_value(value) {
        Ok(v) => CallToolResult::structured(v),
        Err(e) => tool_error(&ToolError::Internal(format!("encode result: {e}"))),
    }
}

impl DwgServer {
    pub fn new(service: Service) -> Self {
        Self {
            service: Arc::new(service),
            gate: Arc::new(Mutex::new(())),
            tool_router: Self::tool_router(),
        }
    }

    /// Run one service call on the blocking pool, holding the gate.
    async fn run<T, F>(&self, f: F) -> Result<T, ToolError>
    where
        T: Send + 'static,
        F: FnOnce(&Service, Now) -> Result<T, ToolError> + Send + 'static,
    {
        let service = Arc::clone(&self.service);
        let gate = Arc::clone(&self.gate);
        tokio::task::spawn_blocking(move || {
            let _guard = gate.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            f(&service, now())
        })
        .await
        .unwrap_or_else(|e| Err(ToolError::Internal(format!("tool task failed: {e}"))))
    }

    async fn call<T, F>(&self, f: F) -> Result<CallToolResult, ErrorData>
    where
        T: Serialize + Send + 'static,
        F: FnOnce(&Service, Now) -> Result<T, ToolError> + Send + 'static,
    {
        Ok(match self.run(f).await {
            Ok(value) => structured(&value),
            Err(e) => tool_error(&e),
        })
    }
}

#[tool_router]
impl DwgServer {
    #[tool(
        name = "parse_file",
        title = "Parse a local DWG or DXF file",
        description = "Parse a DWG or DXF drawing from a local path into page images, readable tiles, a text layer (every label as exact text with its pixel position, no OCR) and a metadata index. Returns a job and a summary; read the details with get_result using job.job_id. Use parse_url instead when the drawing is behind an http or https link. The result is cached for 7 days by file content, size and tiles: parsing the same content again returns job.cached=true at cost_units 0.1 instead of 1 and does not render again. To force a new parse of a file, call reset_file_cache first. A drawing that cannot be parsed returns the error parse_failed, records a failed job and costs 0.",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = false),
        output_schema = schema_for_output::<ParseOutput>()
    )]
    async fn parse_file(
        &self,
        Parameters(input): Parameters<ParseFileInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(move |s, now| s.parse_file(input, now)).await
    }

    #[tool(
        name = "parse_url",
        title = "Parse a DWG or DXF file from a URL",
        description = "Download a DWG or DXF drawing from an http or https URL and parse it; the output is the same as parse_file. Use parse_file instead when the drawing is already on this machine. The download is cached for 7 days by URL, so a repeated call does not fetch the file again (download_cached=true); the parse result is cached for 7 days by file content, so a repeat returns job.cached=true at cost_units 0.1 instead of 1. If the file behind the URL has changed, call reset_url_cache first. Fails with unsupported_file when the link is a web page or not a .dwg or .dxf, too_large above 100 MB, download_failed on an HTTP error or after 30 seconds.",
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = false, open_world_hint = true),
        output_schema = schema_for_output::<ParseOutput>()
    )]
    async fn parse_url(
        &self,
        Parameters(input): Parameters<ParseUrlInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(move |s, now| s.parse_url(input, now)).await
    }

    #[tool(
        name = "get_result",
        title = "Read a parsed drawing",
        description = "Read one part of a parsed drawing, chosen by mode. summary (default): pages, image sizes, label counts, layers and the tile grid; read it first. labels: text labels with pixel positions, filtered by page, layer or query and paged with limit and cursor; use it to find or read text. index: blocks, layouts, entity counts and title-block attributes. page_image: one whole page as a PNG. tile_image: one readable 1500 px tile as a PNG with the labels inside it; use tiles when page text is too small to read. Images are returned as image content up to 5 MB and always as a file path. Reads only the cache: it never parses and costs nothing. Use get_job instead when you only need the status, cost or expiry of a job.",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        output_schema = schema_for_output::<GetResultOutput>()
    )]
    async fn get_result(
        &self,
        Parameters(input): Parameters<GetResultInput>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(match self.run(move |s, now| s.get_result(input, now)).await {
            Ok((out, image)) => {
                let mut result = structured(&out);
                if let Some(image) = image {
                    let data = base64::engine::general_purpose::STANDARD.encode(image.png);
                    result.content.push(ContentBlock::image(data, "image/png"));
                }
                result
            }
            Err(e) => tool_error(&e),
        })
    }

    #[tool(
        name = "get_job",
        title = "Get one parse job",
        description = "Return one parse job by id: status (completed or failed), source, whether it was answered from the cache, cost_units, timings, the error of a failed job, and when its result expires. Use it to check whether a job_id is still valid before reading it. Use get_result instead to read the parsed drawing, and list_jobs when you do not know the job_id. Fails with not_found when the job expired or was deleted.",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        output_schema = schema_for_output::<Job>()
    )]
    async fn get_job(
        &self,
        Parameters(input): Parameters<JobIdInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(move |s, now| s.get_job(&input.job_id, now)).await
    }

    #[tool(
        name = "list_jobs",
        title = "List parse jobs",
        description = "List the parse jobs still in the cache, newest first, with the total number of jobs and the sum of their cost_units. Use it to find a job_id from an earlier parse or to see what was spent; page through with limit and cursor. Use get_job instead when you already have the job_id. Jobs older than 7 days are not listed: they have expired.",
        annotations(read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        output_schema = schema_for_output::<ListJobsOutput>()
    )]
    async fn list_jobs(
        &self,
        Parameters(input): Parameters<ListJobsInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(move |s, now| s.list_jobs(input, now)).await
    }

    #[tool(
        name = "delete_job",
        title = "Delete one parse job",
        description = "Delete one parse job by id. Its cached result (images, tiles, labels, index) is deleted too unless another job still reads the same result; result_removed says which happened. This cannot be undone: the job_id stops working and a later parse of that drawing is a full parse again once the result is gone. Use it to remove a drawing you no longer need. To drop everything cached for a file or a URL regardless of job, use reset_file_cache or reset_url_cache instead.",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false),
        output_schema = schema_for_output::<DeleteJobOutput>()
    )]
    async fn delete_job(
        &self,
        Parameters(input): Parameters<JobIdInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(move |s, now| s.delete_job(&input.job_id, now)).await
    }

    #[tool(
        name = "reset_file_cache",
        title = "Reset the cache of a local file",
        description = "Drop every cached parse result of one local DWG or DXF file, for all sizes and tile settings, together with the jobs that read them, so the next parse_file of it is a full parse at cost_units 1. Use it when a cached result should not be reused, for example after upgrading the server. The file is matched by its current content, so an edited file needs no reset: it already parses fresh. Cannot be undone; calling it again removes nothing. Use reset_url_cache instead for a drawing that was parsed from a URL, and delete_job to remove a single job.",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false),
        output_schema = schema_for_output::<ResetCacheOutput>()
    )]
    async fn reset_file_cache(
        &self,
        Parameters(input): Parameters<ResetFileCacheInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(move |s, now| s.reset_file_cache(input, now)).await
    }

    #[tool(
        name = "reset_url_cache",
        title = "Reset the cache of a URL",
        description = "Drop the cached download of one URL, every parse result of that file and the jobs that read them, so the next parse_url fetches the file again and parses it at cost_units 1. Use it when the file behind the URL has changed within the 7 days the download is cached. Makes no network request. Cannot be undone; calling it again, or for a URL that was never parsed, removes nothing. Use reset_file_cache instead for a local file, and delete_job to remove a single job.",
        annotations(read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false),
        output_schema = schema_for_output::<ResetCacheOutput>()
    )]
    async fn reset_url_cache(
        &self,
        Parameters(input): Parameters<ResetUrlCacheInput>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(move |s, now| s.reset_url_cache(input, now)).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DwgServer {
    fn get_info(&self) -> ServerConfig {
        let mut info = Implementation::default();
        info.name = "dwg-mcp".into();
        info.title = Some("dwg-mcp: DWG and DXF ingestion for AI applications".into());
        info.version = env!("CARGO_PKG_VERSION").into();
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(info)
            .with_instructions(INSTRUCTIONS)
    }
}

/// Load the font, open the cache and serve MCP over stdio until the client leaves.
/// Diagnostics go to stderr; stdout carries only the protocol.
pub fn run(font_path: &str, cache_dir: PathBuf) -> Result<(), String> {
    let font = dwg2png::text::load_font(font_path)?;
    let service = Service::new(cache_dir, font).map_err(|e| e.to_string())?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("start runtime: {e}"))?;
    runtime.block_on(async {
        let running = DwgServer::new(service)
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| format!("start MCP server: {e}"))?;
        running.waiting().await.map_err(|e| format!("MCP server stopped: {e}"))?;
        Ok(())
    })
}
