# Log

## 2026-11-24 - First version: eight tools over stdio

**Goal:** An MCP server for DWG ingestion, as its own crate and program, so it
can be listed in MCP registries and installed apart from the renderer.

**Decisions:**
- Tool names fixed by the owner: `parse_file`, `parse_url`, `get_result`,
  `get_job`, `list_jobs`, `delete_job`, `reset_file_cache`, `reset_url_cache`.
- Downloads and results are cached for 7 days; a cache hit reports
  `cost_units` 0.1 against 1 for a parse. The server reports units and does not
  charge.
- Shape follows the CADLens MCP server: stdio, parse returns a summary and a
  job id, `get_result` reads parts by `mode`.
- Tool definitions follow Glama's Tool Definition Quality Score: a title, a
  description with when to use the tool and which sibling to use instead,
  described parameters, an output schema and all four behaviour hints.
- A damaged drawing is a failed job (`parse_failed`, cost 0). The renderer's
  failsafe mode would otherwise return an empty placeholder page as a success.
- A separate crate that depends on `dwg2png` by pinned commit;
  `dwg2png --mcp` hands over to this binary.

**Verification:** PASS on macOS x64, Rust 1.95.
- `cargo test --release`: 139 end-to-end tests through the `rmcp` client
  against the built binary, 9 unit tests.
- Breaking the cache lifetime (5 days) and the cache price (0.2) on purpose
  made the cache tests fail; the constants were restored.
- `cargo clippy --release --all-targets -- -D warnings`: clean.
- Docker: the image builds (95 MB); a JSON-RPC session over `docker run -i`
  completed the handshake, listed the eight tools, parsed a mounted drawing,
  and a second container on the same cache volume answered from the cache.

**Not verified:** Windows and Linux native builds (the CI and release
workflows have not run yet); scoring by Glama itself; `parse_url` against a
real https host (the tests use a local http server).

## 2026-11-24 - Agent instructions

Added `AGENTS.md` (with `CLAUDE.md` as a symlink to it): how an agent uses the
eight tools (default workflow, paging, tiles, cache and cost, what to do on
each error code) and how to work on this repository. The limits it states were
checked against the constants in `src/contract.rs` and `src/fetch.rs`.

## 2026-11-24 - Binary releases and installers

**Goal:** Install `dwg-mcp` without a Rust toolchain: a shell one-liner,
Homebrew, and PowerShell on Windows.

**Changes:**
- `.github/workflows/release.yml`: builds five targets on a `v*` tag (macOS
  arm64 and x64, Linux x64 and arm64, Windows x64) and attaches each archive
  with its `.sha256`. Both macOS targets build on `macos-latest`.
- `scripts/install.sh` and `scripts/install.ps1`: download the archive of the
  latest release (or `DWG_MCP_TAG`), check its SHA-256, install to
  `DWG_MCP_INSTALL_DIR`. They follow the layout of the `ccbox` installer.
- Homebrew formula in the `aileron-tech/homebrew-dwg-mcp` tap, pointing at the
  release archives.

## 2026-11-24 - 0.1.1: static Linux binaries

**Problem:** The Linux archives of v0.1.0 were linked against the build
runner's glibc (2.43). Installed with the one-liner in `debian:bookworm-slim`,
`ubuntu:22.04` and `ubuntu:24.04` containers, the binary failed to start:
`version GLIBC_2.43 not found`.

**Change:** The two Linux targets are now `x86_64-unknown-linux-musl` and
`aarch64-unknown-linux-musl`: static binaries with no glibc dependency.
`scripts/install.sh` downloads those. No change to the server itself.

**Verification of 0.1.1 (installers run against the published release):**
- Release workflow: all five targets built and attached with checksums.
- `scripts/install.sh` one-liner: installs and runs on macOS x64; in
  `debian:bookworm-slim`, `ubuntu:22.04` and `ubuntu:24.04` containers the
  installed binary also parsed a drawing over MCP stdio.
- `scripts/install.ps1` run under PowerShell 7 on macOS: downloads the Windows
  archive, verifies the checksum and extracts `dwg-mcp.exe`.
- Homebrew: `brew tap aileron-tech/dwg-mcp && brew install dwg-mcp` installs
  0.1.1 on macOS x64; `brew test` and `brew audit --strict` pass. Homebrew 7
  required `brew trust` for the tap first.

**Not verified:** running `dwg-mcp.exe` on Windows and the PATH update in
`install.ps1` (no Windows machine); the macOS arm64 and Linux arm64 binaries
(built by CI, not run).

## 2026-11-24 - 0.2.0: DXF input, cleaner label text

**Changes:**
- `.dxf` files are accepted by `parse_file` and `parse_url`. The format is
  read from the content (`dwg2png::detect_format`), so a DXF named `.dwg`
  parses and a text file named `.dxf` is `unsupported_file`.
- A text DXF that does not end with `EOF` is `parse_failed` before parsing:
  cut in its header or tables it would otherwise come back as an empty
  drawing. A drawing with no entities and a reader error is `parse_failed` too.
- Renderer updated to `dwg2png` 7ea8c53: labels no longer carry MTEXT
  formatting codes or `%%` control codes.
- Tool titles, descriptions and parameter texts name DXF.

**Verification:** PASS. `cargo test --release`: 147 end-to-end tests through
the `rmcp` client (8 new in `tests/mcp_dxf.rs`, REQ-MCP-200 to 207), 1 library
test, 10 unit tests; clippy clean. Six real drawings (four Autodesk DWG
samples, two DXF from open source repositories) were driven through a real
MCP session with the built binary: each parsed in 0.3 to 0.8 s, label search
returned hits, tile images came back as PNG, and a second parse was a cache
hit at 0.1.

- Accept SVG through the local and URL parse tools. Cache vector exports with the existing raster artifacts and expose them through page_svg.

## 2026-11-24 - Claude directory package

- Package the hosted DWG, DXF and SVG parser as a Claude plugin in
  `plugins/dwg-apify`. It connects to the public Apify Actor over HTTPS.

## 2026-11-24 - Authenticated HTTP deployment

- Add a Streamable HTTP entrypoint alongside stdio so the renderer can be
  reached through Aileron's HTTPS hostnames. The HTTP tool surface accepts
  remote HTTPS drawings and excludes local-file and cache management tools.
- Require a bearer token and restrict downloads to public HTTPS destinations.

## 2026-11-24 - Isolate hosted drawing caches by account

Enable an opt-in HTTP mode for the authenticated CAD web gateway. The gateway
supplies a verified account ID, and each account uses a separate cache root.
The HTTP router still offers the same three remote drawing tools.

Verification: HTTP authentication, tool definitions, and URL parsing tests.
The web gateway also exercises same-account caching and cross-account reads
through Docker Compose.
