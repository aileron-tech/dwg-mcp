# dwg-mcp: instructions for agents

dwg-mcp is an MCP server that turns DWG, DXF and SVG drawings into data you can
read: PNG previews, vector SVG pages, readable tiles, every text label with its pixel position,
and a metadata index. This file has two parts: how to use the server's tools,
and how to work on this repository.

## Part 1: using the MCP server

### Connect

```sh
claude mcp add dwg -- dwg-mcp          # Claude Code, native binary
dwg2png --mcp                          # the same server, through dwg2png
docker run --rm -i -v "$PWD/drawings:/work/drawings:ro" -v dwg-mcp-cache:/cache dwg-mcp
```

JSON entry for clients that take one: `{"mcpServers": {"dwg": {"command": "dwg-mcp"}}}`.

In Docker, a drawing's path is the path inside the container
(`/work/drawings/<name>.dwg`), and image paths in results are inside the
container too.

### Default workflow

1. Parse the drawing. Use `parse_file` with an absolute local path. Use
   `parse_url` only when the drawing is behind an http or https link.
2. Keep `job.job_id` from the answer. Read the returned `summary` first: page
   count, page sizes, label counts, layers, and the tile grid of each page.
3. Read only the part you need with `get_result` and that `job_id`:

| You want | Call |
|----------|------|
| To find or read text | `mode: "labels"`, with `query`, `layer` or `page` to narrow it |
| Blocks, layouts, entity counts, title-block attributes | `mode: "index"` |
| To look at a whole page | `mode: "page_image"`, `page` |
| To retrieve a vector page | `mode: "page_svg"`, `page` |
| To read small text in an image | `mode: "tile_image"`, `page`, `row`, `col` |

Prefer `labels` over images for text: labels are the exact strings from the
drawing, not OCR. Use images when layout or geometry matters.

### Rules that save calls

- Pages are numbered from 0. For CAD, page 0 is Model space and the rest are paper sheets. SVG has one page.
- `labels` returns 100 labels per call (`limit` 1 to 500). When the answer has
  `next_cursor`, pass it as `cursor` to continue; when it is absent you have
  everything. `total` is the number of matches over all pages.
- `query` is a case-insensitive substring. `layer` is an exact layer name,
  case-insensitive. Filters combine.
- A label's `x`, `y` is the left end of its text baseline in page-image pixels
  (tile-image pixels in `tile_image`); `w`, `h` its size; `rot` degrees
  clockwise.
- Tiles exist only for pages larger than 1500 px that were parsed with
  `tiles: true` (the default). A page's `tile_rows` and `tile_cols` in the
  summary give the valid `row` and `col`; when they are 0, use `page_image`.
- `size` (512 to 12000, default 3000) is the long edge of the page images.
  Raise it when labels are too small to read even in tiles.
- PNG previews and tiles come back inline up to 5 MB and always include a file path. `page_svg` returns the SVG file path.

### Cache and cost

- A result is cached for 7 days by the drawing's content, `size` and `tiles`.
  Parsing the same drawing again returns `job.cached: true` and
  `cost_units: 0.1` instead of 1. A failed parse costs 0.
- `parse_url` also caches the download for 7 days by URL
  (`download_cached: true`), so it does not see a file that changed behind the
  same URL.
- `get_result`, `get_job` and `list_jobs` never parse and cost nothing.
- Default: reuse the cache. Reset it only in these cases, then parse again:
  - the file behind a URL changed: `reset_url_cache` with that URL;
  - a cached result of a local file must not be reused: `reset_file_cache`
    with its path. An edited local file needs no reset: its content changed,
    so it already parses fresh.
- `delete_job` removes one job, and its result when no other job reads it.
  Use it only when the user wants a drawing removed. It cannot be undone.
- `list_jobs` finds a `job_id` from an earlier parse and shows the total cost.
  A job older than 7 days is gone: parse the drawing again.

### Errors

A failed call has `isError: true` and a text `code: message`.

| Code | Meaning | What to do |
|------|---------|------------|
| `invalid_argument` | A value is out of range or missing; the message names it | Fix the argument and call again |
| `not_found` | No such file, or the job expired or was deleted | Check the path, or parse again |
| `unsupported_file` | Not a `.dwg`, `.dxf` or `.svg`, not a drawing inside, or the URL is a web page | Tell the user; DWF and other formats are not supported |
| `too_large` | Over 100 MB | Tell the user |
| `download_failed` | HTTP error, unreachable host, or 30 s timeout | Check the URL; retry once |
| `parse_failed` | The drawing is damaged, a DXF was cut short, or the DWG version is unsupported | Tell the user; it was not billed |

### Example

```
parse_file  {"path": "/work/drawings/heat-network.dwg"}
  -> job.job_id = "job_97dc2f06dfe3e2da", summary.page_count = 2

get_result  {"job_id": "job_97dc2f06dfe3e2da", "mode": "labels", "query": "DN150"}
  -> labels: [{"text": "PIPE DN150", "page": 0, "layer": "PIPES", "x": 2416, "y": 1050, ...}]

get_result  {"job_id": "job_97dc2f06dfe3e2da", "mode": "tile_image", "page": 0, "row": 0, "col": 1}
  -> the tile as a PNG, plus the labels inside it
```

## Part 2: working on this repository

### Layout

| Path | Role |
|------|------|
| `src/contract.rs` | Wire types of the eight tools and the limits. The public contract. |
| `src/domain.rs` | Pure rules: validation, cache keys, expiry, cost, paging. No I/O, no clock. |
| `src/store.rs` | Filesystem adapter: jobs, results, downloads. |
| `src/fetch.rs` | HTTP adapter: download with size cap and time limit. |
| `src/service.rs` | Orchestration of each tool. |
| `src/server.rs` | MCP entrypoint: tool definitions and descriptions, the clock, stdio. |
| `tests/mcp_*.rs` | End-to-end tests through the `rmcp` client against the built binary. |
| `docs/requirements.md` | Requirement register, generated from the test tags. |

The renderer is the `dwg2png` crate, pinned by git revision in `Cargo.toml`.
`dwg-apify` depends on this crate's library; `Service` and `contract` are its
API too.

### Rules

- Tool names, parameter names, result fields and error codes are a public
  contract. Change them only when the owner asks, and say what breaks.
- A tool description states what the tool does, when to use it, and which
  sibling tool to use instead. `tests/mcp_definitions.rs` checks this.
- Keep `domain.rs` pure. Read the time once in `server.rs` and pass it down.
- Every test carries `/* REQ-MCP-<id>: <description> */`. Take the next free
  id in the tool's block (see `docs/requirements.md`), then run
  `python3 scripts/gen_requirements.py`.
- Test drawings are written at test time by `tests/common/fixtures.rs`. Do not
  commit DWG files.
- To test expiry, age the cache with `Harness::age_cache(days)`; there is no
  clock override in the program.
- When a change is done, append what changed and why to `LOG.md`.

### Commands

```sh
cargo test --release                                   # all tests
cargo test --release --test mcp_cache                  # one group
cargo clippy --release --all-targets -- -D warnings    # lint
docker build -t dwg-mcp .                              # image
```
