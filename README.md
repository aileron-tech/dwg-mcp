# dwg-mcp

**MCP server for DWG and DXF ingestion.** dwg-mcp gives an AI agent eight tools
to turn an AutoCAD drawing (DWG R13-R2018, or DXF) into data it can use: page images, readable
tiles, every text label with its pixel position (read from the DWG, no OCR) and
a metadata index. It runs locally over stdio, offline except for `parse_url`.

The rendering is done by [dwg2png](https://github.com/aileron-tech/dwg2png);
this repository is the server around it.

## Tools

| Tool | What it does |
|------|--------------|
| `parse_file` | Parse a DWG or DXF from a local path. Returns a job and a summary. |
| `parse_url` | Download a DWG or DXF from an http or https URL and parse it. |
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

`.dwg` and `.dxf` files are accepted (up to 100 MB); the format is read from
the content of the file. DWF and other files are refused with
`unsupported_file`. A damaged DWG and a DXF that was cut short fail with
`parse_failed` instead of returning an empty drawing.

## Run it

### Install the binary

macOS and Linux (Apple Silicon, Intel, x64, arm64):

```sh
curl -fsSL https://raw.githubusercontent.com/aileron-tech/dwg-mcp/main/scripts/install.sh | /bin/bash
```

Homebrew (macOS and Linux):

```sh
brew tap aileron-tech/dwg-mcp
brew install dwg-mcp
```

If Homebrew says the tap is untrusted, run `brew trust aileron-tech/dwg-mcp`
once and install again.

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/aileron-tech/dwg-mcp/main/scripts/install.ps1 | iex
```

The scripts download the archive for your platform from the latest
[release](https://github.com/aileron-tech/dwg-mcp/releases), check its SHA-256
and put `dwg-mcp` in `~/.local/bin` (Windows:
`%LOCALAPPDATA%\Programs\dwg-mcp`, added to your PATH). Set
`DWG_MCP_INSTALL_DIR` to change the folder and `DWG_MCP_TAG` to pin a version.

From source (Rust 1.88 or newer):

```sh
cargo install --git https://github.com/aileron-tech/dwg-mcp
```

With [dwg2png](https://github.com/aileron-tech/dwg2png) installed too,
`dwg2png --mcp` starts the same server: it looks for `dwg-mcp` next to the
`dwg2png` executable and then on `PATH`.

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

147 end-to-end tests spawn the built `dwg-mcp` binary and drive it with the
official Rust MCP client (`rmcp`) over stdio; a small local HTTP server stands
in for remote files. Drawings are synthetic, written at test time; none is
committed. One test covers the library accessor used by the Apify Actor, and
ten unit tests cover the pure rules. Each test carries a
requirement id; the register is [docs/requirements.md](docs/requirements.md).

The tool definitions are tested as data (titles, descriptions that name the
alternative tool, described parameters, output schemas, behaviour hints),
following the [Tool Definition Quality Score](https://tdqs.dev/spec) rubric
that Glama uses to rank MCP servers.

## License

MPL-2.0
