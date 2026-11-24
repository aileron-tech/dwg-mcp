//! dwg-mcp: MCP server for DWG, DXF and SVG ingestion, over stdio.
//!
//! Usage:
//!   dwg-mcp              serve MCP on stdin/stdout (also: `dwg2png --mcp`)
//!   dwg-mcp --version
//!   dwg-mcp --help
//!
//! Environment:
//!   DWG_MCP_CACHE_DIR    cache folder (default: the user's cache folder)
//!   DWG2PNG_FONT         TrueType font used to draw text

use std::path::{Path, PathBuf};

const HELP: &str = "dwg-mcp: MCP server for DWG, DXF and SVG ingestion (stdio)

Usage:
  dwg-mcp              serve MCP on stdin/stdout
  dwg-mcp --version    print the version
  dwg-mcp --help       print this text

Environment:
  DWG_MCP_CACHE_DIR    cache folder for downloads and results, kept 7 days
                       (default: $XDG_CACHE_HOME/dwg-mcp or ~/.cache/dwg-mcp)
  DWG2PNG_FONT         TrueType font used to draw text

Tools: parse_file, parse_url, get_result, get_job, list_jobs, delete_job,
reset_file_cache, reset_url_cache.";

/// Fonts tried when DWG2PNG_FONT is not set: macOS, Debian/Ubuntu, Fedora, Windows.
const FONT_CANDIDATES: [&str; 4] = [
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
];

fn font_path() -> Result<String, String> {
    if let Ok(font) = std::env::var("DWG2PNG_FONT") {
        return Ok(font);
    }
    FONT_CANDIDATES
        .iter()
        .find(|p| Path::new(p).is_file())
        .map(|p| p.to_string())
        .ok_or_else(|| "no TrueType font found; set DWG2PNG_FONT to a .ttf file".to_string())
}

fn cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DWG_MCP_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("dwg-mcp");
    }
    match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        Ok(home) => PathBuf::from(home).join(".cache").join("dwg-mcp"),
        Err(_) => std::env::temp_dir().join("dwg-mcp-cache"),
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("--version") | Some("-V") => {
            println!("dwg-mcp {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("--help") | Some("-h") => {
            println!("{HELP}");
            return;
        }
        Some(other) => {
            eprintln!("dwg-mcp: unknown argument {other:?}\n\n{HELP}");
            std::process::exit(2);
        }
    }
    let outcome = font_path().and_then(|font| dwg_mcp::server::run(&font, cache_dir()));
    if let Err(e) = outcome {
        eprintln!("dwg-mcp: {e}");
        std::process::exit(1);
    }
}
