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
