# dwg-mcp

**MCP server for DWG ingestion.** dwg-mcp gives an AI agent eight tools to turn
an AutoCAD DWG drawing (R13-R2018) into data it can use: page images, readable
tiles, every text label with its pixel position (read from the DWG, no OCR) and
a metadata index. It runs locally over stdio, offline except for `parse_url`.

The rendering is done by [dwg2png](https://github.com/aileron-tech/dwg2png);
this repository is the server around it.

## Tools

| Tool | What it does |
|------|--------------|
| `parse_file` | Parse a DWG from a local path. Returns a job and a summary. |
| `parse_url` | Download a DWG from an http or https URL and parse it. |
| `get_result` | Read one part of a parsed drawing: `summary`, `labels`, `index`, `page_image` or `tile_image`. |
| `get_job` | Status, cost and expiry of one job. |
| `list_jobs` | Jobs in the cache, newest first, with the total cost. |
| `delete_job` | Delete one job and, when no other job reads it, its result. |
| `reset_file_cache` | Drop every cached result of a local file. |
| `reset_url_cache` | Drop the cached download and results of a URL. |

A typical session: `parse_file` -> `get_result` (summary) -> `get_result` with
`mode: "labels"` and a `query`, or `mode: "tile_image"` for a readable crop.

### Cache and cost

- A **download** is cached for 7 days by URL: a repeated `parse_url` does not
  fetch the file again.
- A **result** is cached for 7 days by the drawing's content, `size` and
  `tiles`: the same drawing is a cache hit whether it arrives by path or URL.
- Every job reports `cost_units`: **1** for a parse, **0.1** for a cache hit,
  **0** for a failed parse. The server only reports the units; charging is up to
  whatever hosts it.
- A cache hit does not extend the 7 days. `reset_file_cache` and
  `reset_url_cache` force the next parse to be a full one.

### Errors

A failed call returns `isError: true` with a text of the form `code: message`.
Codes: `invalid_argument`, `not_found`, `unsupported_file`, `too_large`,
`download_failed`, `parse_failed`, `internal`.

Only `.dwg` files are accepted (up to 100 MB). DXF and DWF are refused with
`unsupported_file`.

## Run it

### Native binary

```sh
# from source (Rust 1.88 or newer)
cargo install --git https://github.com/aileron-tech/dwg-mcp

dwg-mcp --version
```

With [dwg2png](https://github.com/aileron-tech/dwg2png) installed too,
`dwg2png --mcp` starts the same server: it looks for `dwg-mcp` next to the
`dwg2png` executable and then on `PATH`.

Prebuilt archives for macOS (Apple Silicon and Intel), Linux x64 and Windows
x64 are attached to each release by `.github/workflows/release.yml`. Homebrew
and Scoop manifests will point at those archives; until the first release, use
`cargo install`.

### Docker

```sh
docker build -t dwg-mcp .

# MCP over stdio: -i is required. Mount the drawings and a cache volume.
docker run --rm -i \
  -v "$PWD/drawings:/work/drawings:ro" \
  -v dwg-mcp-cache:/cache \
  dwg-mcp
```

Inside the container a drawing is `/work/drawings/<name>.dwg`; pass that path to
`parse_file`. The image paths returned by `get_result` are inside the container
(`/cache/...`); images up to 5 MB are also returned as image content.

### Connect a client

Claude Code:

```sh
claude mcp add dwg -- dwg-mcp
```

Claude Desktop, Cursor and other clients that take a JSON entry:

```json
{
  "mcpServers": {
    "dwg": { "command": "dwg-mcp" }
  }
}
```

Docker instead of the binary:

```json
{
  "mcpServers": {
    "dwg": {
      "command": "docker",
      "args": ["run", "--rm", "-i",
               "-v", "/path/to/drawings:/work/drawings:ro",
               "-v", "dwg-mcp-cache:/cache", "dwg-mcp"]
    }
  }
}
```

### Configuration

| Variable | Meaning | Default |
|----------|---------|---------|
| `DWG_MCP_CACHE_DIR` | Folder for downloads and results | `$XDG_CACHE_HOME/dwg-mcp`, else `~/.cache/dwg-mcp` |
| `DWG2PNG_FONT` | TrueType font used to draw text | Arial on macOS and Windows, DejaVu Sans on Linux |

## Architecture

```
server   (MCP entrypoint: tool definitions, clock, stdio loop)
  -> service   (orchestration: validate, consult cache, render, record job)
       -> domain    (pure rules: validation, cache keys, expiry, cost, paging)
       -> store     (filesystem adapter: jobs, results, downloads)
       -> fetch     (HTTP adapter: download with size cap and time limit)
       -> dwg2png   (the renderer, a library dependency)
contract (wire types shared by all of the above)
```

`domain` has no I/O and no clock. The time of a call is read once in `server`
and passed down.

## Tests

```sh
cargo test --release
```

139 end-to-end tests spawn the built `dwg-mcp` binary and drive it with the
official Rust MCP client (`rmcp`) over stdio; a small local HTTP server stands
in for remote files. Drawings are synthetic, written at test time; none is
committed. One test covers the library accessor used by the Apify Actor, and
nine unit tests cover the pure rules. Each test carries a
requirement id; the register is [docs/requirements.md](docs/requirements.md).

The tool definitions are tested as data (titles, descriptions that name the
alternative tool, described parameters, output schemas, behaviour hints),
following the [Tool Definition Quality Score](https://tdqs.dev/spec) rubric
that Glama uses to rank MCP servers.

## License

MPL-2.0
