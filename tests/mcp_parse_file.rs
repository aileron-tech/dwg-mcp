//! parse_file: what a parse returns, and which inputs it refuses.

mod common;

use common::{fixtures, job_id, strs, unix_of, Harness};
use serde_json::json;
use sha2::{Digest, Sha256};

/* REQ-MCP-030: parse_file returns a completed job with a job_id */
#[tokio::test]
async fn returns_a_completed_job() {
    let h = Harness::start().await;
    let out = h.parse_file(&h.plan()).await;
    assert_eq!(out["job"]["status"], "completed");
    assert!(job_id(&out).starts_with("job_") && job_id(&out).len() == 20);
    assert!(out["job"].get("error").is_none());
    h.stop().await;
}

/* REQ-MCP-031: a first parse is not cached and costs one unit */
#[tokio::test]
async fn first_parse_costs_one_unit() {
    let h = Harness::start().await;
    let out = h.parse_file(&h.plan()).await;
    assert_eq!(out["job"]["cached"], false);
    assert_eq!(out["job"]["cost_units"], 1.0);
    assert!(out.get("download_cached").is_none(), "download_cached belongs to parse_url");
    h.stop().await;
}

/* REQ-MCP-032: the summary lists Model space first, then the paper sheets */
#[tokio::test]
async fn summary_lists_model_space_and_sheets() {
    let h = Harness::start().await;
    let summary = h.parse_file(&h.plan()).await["summary"].clone();
    assert_eq!(summary["page_count"], 2);
    assert_eq!(summary["pages"][0]["index"], 0);
    assert_eq!(summary["pages"][0]["name"], "Model");
    assert_eq!(summary["pages"][1]["index"], 1);
    assert_eq!(summary["pages"][1]["name"], "Layout1");
    h.stop().await;
}

/* REQ-MCP-033: the summary counts labels per page and in total */
#[tokio::test]
async fn summary_counts_labels() {
    let h = Harness::start().await;
    let summary = h.parse_file(&h.plan()).await["summary"].clone();
    assert_eq!(summary["pages"][0]["label_count"], fixtures::PLAN_LABEL_COUNT);
    let per_page: u64 =
        summary["pages"].as_array().unwrap().iter().map(|p| p["label_count"].as_u64().unwrap()).sum();
    assert_eq!(summary["label_count"].as_u64().unwrap(), per_page);
    h.stop().await;
}

/* REQ-MCP-034: the summary names the layers and the DWG version */
#[tokio::test]
async fn summary_names_layers_and_version() {
    let h = Harness::start().await;
    let summary = h.parse_file(&h.plan()).await["summary"].clone();
    let layers = strs(&summary["layers"]);
    assert!(layers.contains(&"WALLS".to_string()) && layers.contains(&"PIPES".to_string()));
    assert_eq!(summary["layer_count"].as_u64().unwrap() as usize, layers.len());
    assert_eq!(summary["version"], "AC1032");
    assert_eq!(summary["file_name"], "plan.dwg");
    h.stop().await;
}

/* REQ-MCP-035: by default pages are 3000 px on the long edge, with tiles */
#[tokio::test]
async fn defaults_are_3000_px_with_tiles() {
    let h = Harness::start().await;
    let summary = h.parse_file(&h.plan()).await["summary"].clone();
    assert_eq!(summary["size"], 3000);
    assert_eq!(summary["tiles"], true);
    let page = &summary["pages"][0];
    assert_eq!(page["width"].as_u64().unwrap().max(page["height"].as_u64().unwrap()), 3000);
    assert!(page["tile_rows"].as_u64().unwrap() >= 1 && page["tile_cols"].as_u64().unwrap() >= 2);
    h.stop().await;
}

/* REQ-MCP-036: size sets the long edge of the page images */
#[tokio::test]
async fn size_sets_the_long_edge() {
    let h = Harness::start().await;
    let out = h.call("parse_file", json!({ "path": h.plan(), "size": 1024 })).await.ok();
    let page = &out["summary"]["pages"][0];
    assert_eq!(out["summary"]["size"], 1024);
    assert_eq!(page["width"].as_u64().unwrap().max(page["height"].as_u64().unwrap()), 1024);
    h.stop().await;
}

/* REQ-MCP-037: a size outside 512..12000 is refused before any parsing */
#[tokio::test]
async fn size_out_of_range_is_refused() {
    let h = Harness::start().await;
    for size in [0, 511, 12001] {
        let reply = h.call("parse_file", json!({ "path": h.plan(), "size": size })).await;
        assert_eq!(reply.err_code(), "invalid_argument", "size {size}");
    }
    assert!(h.job_ids().await.is_empty(), "a refused call must not record a job");
    h.stop().await;
}

/* REQ-MCP-038: the size limits themselves are accepted */
#[tokio::test]
async fn size_limits_are_inclusive() {
    let h = Harness::start().await;
    let small = h.call("parse_file", json!({ "path": h.blank(), "size": 512, "tiles": false })).await.ok();
    assert_eq!(small["summary"]["size"], 512);
    h.stop().await;
}

/* REQ-MCP-039: tiles=false produces no tile grid */
#[tokio::test]
async fn tiles_false_produces_no_tiles() {
    let h = Harness::start().await;
    let out = h.call("parse_file", json!({ "path": h.plan(), "size": 1024, "tiles": false })).await.ok();
    assert_eq!(out["summary"]["tiles"], false);
    for page in out["summary"]["pages"].as_array().unwrap() {
        assert_eq!((page["tile_rows"].as_u64(), page["tile_cols"].as_u64()), (Some(0), Some(0)));
    }
    h.stop().await;
}

/* REQ-MCP-040: a relative path is refused */
#[tokio::test]
async fn relative_path_is_refused() {
    let h = Harness::start().await;
    let reply = h.call("parse_file", json!({ "path": "drawings/plan.dwg" })).await;
    assert_eq!(reply.err_code(), "invalid_argument");
    assert!(reply.text.contains("absolute"));
    h.stop().await;
}

/* REQ-MCP-041: a path with no file is not_found */
#[tokio::test]
async fn missing_file_is_not_found() {
    let h = Harness::start().await;
    let reply = h.call("parse_file", json!({ "path": h.files_dir().join("nope.dwg") })).await;
    assert_eq!(reply.err_code(), "not_found");
    h.stop().await;
}

/* REQ-MCP-042: a directory is refused */
#[tokio::test]
async fn directory_is_refused() {
    let h = Harness::start().await;
    let reply = h.call("parse_file", json!({ "path": h.files_dir() })).await;
    assert_eq!(reply.err_code(), "invalid_argument");
    h.stop().await;
}

/* REQ-MCP-043: a file that is not named .dwg is unsupported */
#[tokio::test]
async fn other_extension_is_unsupported() {
    let h = Harness::start().await;
    let dxf = h.files_dir().join("plan.dxf");
    std::fs::copy(h.plan(), &dxf).unwrap();
    let reply = h.call("parse_file", json!({ "path": dxf })).await;
    assert_eq!(reply.err_code(), "unsupported_file");
    h.stop().await;
}

/* REQ-MCP-044: a .dwg file without a DWG version code is unsupported */
#[tokio::test]
async fn non_dwg_content_is_unsupported() {
    let h = Harness::start().await;
    let fake = h.files_dir().join("fake.dwg");
    std::fs::write(&fake, b"%PDF-1.7 this is a renamed PDF").unwrap();
    assert_eq!(h.call("parse_file", json!({ "path": fake })).await.err_code(), "unsupported_file");
    let empty = h.files_dir().join("empty.dwg");
    std::fs::write(&empty, b"").unwrap();
    assert_eq!(h.call("parse_file", json!({ "path": empty })).await.err_code(), "unsupported_file");
    h.stop().await;
}

/* REQ-MCP-045: the .dwg extension is matched without regard to case */
#[tokio::test]
async fn uppercase_extension_is_accepted() {
    let h = Harness::start().await;
    let upper = h.files_dir().join("SITE PLAN.DWG");
    std::fs::copy(h.plan(), &upper).unwrap();
    let out = h.call("parse_file", json!({ "path": upper, "size": 1024 })).await.ok();
    assert_eq!(out["summary"]["file_name"], "SITE PLAN.DWG");
    h.stop().await;
}

/* REQ-MCP-046: the job records its source path and the SHA-256 of the file */
#[tokio::test]
async fn job_records_source_and_content_hash() {
    let h = Harness::start().await;
    let path = h.plan();
    let out = h.parse_small(&path).await;
    assert_eq!(out["job"]["source"]["kind"], "file");
    assert_eq!(out["job"]["source"]["value"], path.to_str().unwrap());
    let expected: String =
        Sha256::digest(std::fs::read(&path).unwrap()).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(out["job"]["sha256"], expected);
    h.stop().await;
}

/* REQ-MCP-047: a result expires exactly seven days after it was created */
#[tokio::test]
async fn result_expires_seven_days_after_creation() {
    let h = Harness::start().await;
    let out = h.parse_small(&h.plan()).await;
    let created = unix_of(out["job"]["created_at"].as_str().unwrap());
    let expires = unix_of(out["job"]["expires_at"].as_str().unwrap());
    assert_eq!(expires - created, 7 * 24 * 60 * 60);
    h.stop().await;
}

/* REQ-MCP-048: a drawing without text parses with zero labels */
#[tokio::test]
async fn drawing_without_text_has_no_labels() {
    let h = Harness::start().await;
    let out = h.parse_small(&h.blank()).await;
    assert_eq!(out["job"]["status"], "completed");
    assert_eq!(out["summary"]["label_count"], 0);
    assert!(out["summary"]["entity_count"].as_u64().unwrap() > 0);
    h.stop().await;
}

/* REQ-MCP-049: a damaged drawing fails with parse_failed instead of a placeholder */
#[tokio::test]
async fn damaged_drawing_fails_to_parse() {
    let h = Harness::start().await;
    let whole = std::fs::read(h.plan()).unwrap();
    let damaged = h.files_dir().join("damaged.dwg");
    std::fs::write(&damaged, &whole[..1000]).unwrap();
    let reply = h.call("parse_file", json!({ "path": damaged })).await;
    assert_eq!(reply.err_code(), "parse_failed");
    assert!(reply.text.contains("damaged.dwg"));
    h.stop().await;
}

/* REQ-MCP-050: a failed parse is recorded as a failed job that costs nothing */
#[tokio::test]
async fn failed_parse_is_recorded_and_free() {
    let h = Harness::start().await;
    let whole = std::fs::read(h.plan()).unwrap();
    let damaged = h.files_dir().join("damaged.dwg");
    std::fs::write(&damaged, &whole[..1000]).unwrap();
    assert!(h.call("parse_file", json!({ "path": damaged })).await.is_error);
    let listing = h.call("list_jobs", json!({})).await.ok();
    assert_eq!(listing["total"], 1);
    assert_eq!(listing["total_cost_units"], 0.0);
    let job = &listing["jobs"][0];
    assert_eq!(job["status"], "failed");
    assert_eq!(job["cost_units"], 0.0);
    assert!(job["error"].as_str().unwrap().contains("damaged"));
    let id = job["job_id"].as_str().unwrap();
    let read = h.call("get_result", json!({ "job_id": id })).await;
    assert_eq!(read.err_code(), "parse_failed");
    h.stop().await;
}

/* REQ-MCP-051: a failed parse is not cached: the next attempt parses again */
#[tokio::test]
async fn failed_parse_is_not_cached() {
    let h = Harness::start().await;
    let whole = std::fs::read(h.plan()).unwrap();
    let damaged = h.files_dir().join("damaged.dwg");
    std::fs::write(&damaged, &whole[..1000]).unwrap();
    assert!(h.call("parse_file", json!({ "path": damaged })).await.is_error);
    assert!(h.call("parse_file", json!({ "path": damaged })).await.is_error);
    assert_eq!(h.job_ids().await.len(), 2);
    h.stop().await;
}

/* REQ-MCP-052: the text content of a result is the same JSON as the structured content */
#[tokio::test]
async fn text_content_mirrors_structured_content() {
    let h = Harness::start().await;
    let reply = h.call("parse_file", json!({ "path": h.blank(), "size": 512 })).await;
    let from_text: serde_json::Value = serde_json::from_str(&reply.text).unwrap();
    assert_eq!(from_text, reply.data);
    h.stop().await;
}
