# dwg-mcp

**MCP server for DWG, DXF and SVG ingestion.** dwg-mcp gives an AI agent eight tools
to turn a DWG, DXF or SVG drawing into data it can use: PNG previews, vector SVG pages, readable
tiles, every text label with its pixel position (read from the drawing, no OCR) and
a metadata index. It runs locally over stdio, offline except for `parse_url`.

The rendering is done by [dwg2png](https://github.com/aileron-tech/dwg2png);
this repository is the server around it.

## Use via Apify

The hosted [dwg-apify Actor](https://apify.com/aileron-tech/dwg-apify) processes batches of
DWG, DXF, and SVG URLs. Each URL produces one Apify dataset item with its
status and links to successful outputs, including PNG previews and SVG pages.
Sign in to Apify and pass the drawing URLs to the `aileron-tech--dwg-apify`
tool. [Hosted pricing](https://apify.com/aileron-tech/dwg-apify/pricing)

### Client setup

Use this Actor-specific server URL. Select Streamable HTTP and OAuth in
clients that ask for a transport and authentication method, then sign in to Apify.

```text
https://mcp.apify.com/?tools=actors,docs,aileron-tech/dwg-apify
```

| Client | Setup |
|--------|-------|
| General | Add the URL as a remote MCP server and complete Apify sign-in. [Apify MCP setup](https://docs.apify.com/mcp) |
| ChatGPT | Enable developer mode if required, create a custom MCP app, and enter the URL with OAuth. [ChatGPT guide](https://docs.apify.com/integrations/chatgpt) |
| Claude | In **Customize > Connectors**, add a custom connector with the URL. [Claude guide](https://docs.apify.com/integrations/claude-ai) |
| Claude Desktop | Add the URL under **Customize > Connectors** as a remote connector. [Claude Desktop guide](https://docs.apify.com/integrations/claude-desktop) |
| Claude Code | Run the command below, then use `/mcp` to connect and sign in. [Claude Code guide](https://code.claude.com/docs/en/mcp) |
| Codex | Run the command below or add the URL as a remote MCP server in the app. [Codex CLI guide](https://docs.apify.com/integrations/codex-cli) and [Codex app guide](https://docs.apify.com/integrations/codex-app) |
| Cursor | Add a remote MCP server in Cursor settings and use the URL. [Cursor guide](https://docs.apify.com/integrations/cursor) |
| OpenCode | Merge the example below into `opencode.json`. [OpenCode guide](https://docs.apify.com/integrations/opencode) |
| VS Code | Merge the example below into `.vscode/mcp.json`, then start the server and sign in when prompted. [VS Code guide](https://docs.apify.com/integrations/vscode) |

**Claude Code**

```sh
claude mcp add --transport http dwg-apify 'https://mcp.apify.com/?tools=actors,docs,aileron-tech/dwg-apify'
```

**Codex CLI**

```sh
codex mcp add dwg-apify --url 'https://mcp.apify.com/?tools=actors,docs,aileron-tech/dwg-apify'
codex mcp login dwg-apify
```

**OpenCode**

```json
{
  "mcp": {
    "dwg-apify": {
      "type": "remote",
      "url": "https://mcp.apify.com/?tools=actors,docs,aileron-tech/dwg-apify"
    }
  }
}
```

If OpenCode prompts for authentication, run `opencode mcp auth dwg-apify`.

**VS Code**

```json
{
  "servers": {
    "dwg-apify": {
      "type": "http",
      "url": "https://mcp.apify.com/?tools=actors,docs,aileron-tech/dwg-apify"
    }
  }
}
```

For more clients, use the Apify guide for
[Antigravity](https://docs.apify.com/integrations/antigravity),
[DeepSeek Harness](https://docs.apify.com/integrations/deepseek-harness),
[GitHub Copilot](https://docs.apify.com/integrations/github-copilot),
[Grok Bot](https://docs.apify.com/integrations/grok-bot),
[Grok Build](https://docs.apify.com/integrations/grok-build),
[Hermes Agent](https://docs.apify.com/integrations/hermes-agent),
[Kimi Code CLI](https://docs.apify.com/integrations/kimi-code-cli),
[Lovable](https://docs.apify.com/integrations/lovable),
[Manus](https://docs.apify.com/integrations/manus),
[OpenClaw](https://docs.apify.com/integrations/openclaw), or
[Qoder and Qwen](https://docs.apify.com/integrations/alibaba).
Use the Actor-specific URL in clients that accept a custom server URL.
[Apify's integration directory](https://docs.apify.com/integrations/agents)
lists current options.

Try this prompt after connecting.

> Use dwg-apify to parse https://download.autodesk.com/us/samplefiles/acad/blocks_and_tables_-_imperial.dwg and report the page count and the number of native text labels.

## Local MCP tools

| Tool | What it does |
|------|--------------|
| `parse_file` | Parse a DWG, DXF or SVG from a local path. Returns a job and a summary. |
| `parse_url` | Download a DWG, DXF or SVG from an http or https URL and parse it. |
| `get_result` | Read one part of a parsed drawing: `summary`, `labels`, `index`, `page_image`, `page_svg` or `tile_image`. |
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

`.dwg`, `.dxf`, and `.svg` files are accepted (up to 100 MB); the format is read from
the content of the file. DWF and other files are refused with
`unsupported_file`. Damaged drawings and truncated DXF or SVG fail with
`parse_failed` instead of returning an empty drawing.

To retrieve a vector page for any of the three input formats, call
`get_result` with `mode: "page_svg"` and a zero-based `page`. Read the
SVG file from `image.path`. Continue using `page_image` and `tile_image`
for PNG previews and tiles. Previously cached jobs can still serve their PNGs.
Parse the drawing again to generate SVG pages for those jobs.

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

### Connect a local client

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

152 end-to-end tests spawn the built `dwg-mcp` binary and drive it with the
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
