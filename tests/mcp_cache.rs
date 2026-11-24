//! The result cache: seven days, keyed by content and options, ten times
//! cheaper on a hit; and reset_file_cache.

mod common;

use common::{fixtures, job_id, Harness};
use serde_json::json;

/* REQ-MCP-060: parsing the same file again is answered from the cache */
#[tokio::test]
async fn second_parse_is_cached() {
    let h = Harness::start().await;
    let path = h.plan();
    assert_eq!(h.parse_small(&path).await["job"]["cached"], false);
    assert_eq!(h.parse_small(&path).await["job"]["cached"], true);
    h.stop().await;
}

/* REQ-MCP-061: a cache hit costs one tenth of a fresh parse */
#[tokio::test]
async fn cache_hit_is_ten_times_cheaper() {
    let h = Harness::start().await;
    let path = h.plan();
    let fresh = h.parse_small(&path).await["job"]["cost_units"].as_f64().unwrap();
    let hit = h.parse_small(&path).await["job"]["cost_units"].as_f64().unwrap();
    assert_eq!(fresh, 1.0);
    assert_eq!(hit, 0.1);
    assert!((fresh / hit - 10.0).abs() < 1e-9);
    h.stop().await;
}

/* REQ-MCP-062: a cache hit is a new job with the same summary */
#[tokio::test]
async fn cache_hit_is_a_new_job_with_the_same_summary() {
    let h = Harness::start().await;
    let path = h.plan();
    let first = h.parse_small(&path).await;
    let second = h.parse_small(&path).await;
    assert_ne!(job_id(&first), job_id(&second));
    assert_eq!(first["summary"], second["summary"]);
    assert_eq!(h.job_ids().await.len(), 2);
    h.stop().await;
}

/* REQ-MCP-063: the cache is keyed by content, not by path or file name */
#[tokio::test]
async fn cache_is_keyed_by_content() {
    let h = Harness::start().await;
    let path = h.plan();
    h.parse_small(&path).await;
    let copy = h.files_dir().join("renamed copy.dwg");
    std::fs::copy(&path, &copy).unwrap();
    let out = h.parse_small(&copy).await;
    assert_eq!(out["job"]["cached"], true);
    assert_eq!(out["summary"]["file_name"], "renamed copy.dwg");
    assert_eq!(out["job"]["file_name"], "renamed copy.dwg");
    h.stop().await;
}

/* REQ-MCP-064: a different size is a different cache entry */
#[tokio::test]
async fn different_size_is_a_fresh_parse() {
    let h = Harness::start().await;
    let path = h.plan();
    h.call("parse_file", json!({ "path": path, "size": 1024 })).await.ok();
    let other = h.call("parse_file", json!({ "path": path, "size": 800 })).await.ok();
    assert_eq!(other["job"]["cached"], false);
    assert_eq!(other["job"]["cost_units"], 1.0);
    h.stop().await;
}

/* REQ-MCP-065: a different tiles setting is a different cache entry */
#[tokio::test]
async fn different_tiles_setting_is_a_fresh_parse() {
    let h = Harness::start().await;
    let path = h.plan();
    h.call("parse_file", json!({ "path": path, "size": 1024, "tiles": true })).await.ok();
    let other = h.call("parse_file", json!({ "path": path, "size": 1024, "tiles": false })).await.ok();
    assert_eq!(other["job"]["cached"], false);
    h.stop().await;
}

/* REQ-MCP-066: omitted options and their explicit defaults are the same entry */
#[tokio::test]
async fn explicit_defaults_hit_the_default_entry() {
    let h = Harness::start().await;
    let path = h.blank();
    h.call("parse_file", json!({ "path": path })).await.ok();
    let explicit = h.call("parse_file", json!({ "path": path, "size": 3000, "tiles": true })).await.ok();
    assert_eq!(explicit["job"]["cached"], true);
    h.stop().await;
}

/* REQ-MCP-067: an edited file at the same path parses fresh */
#[tokio::test]
async fn edited_file_parses_fresh() {
    let h = Harness::start().await;
    let path = h.plan();
    let before = h.parse_small(&path).await;
    fixtures::write_dwg(&fixtures::dense_document(30), h.files_dir(), "plan.dwg");
    let after = h.parse_small(&path).await;
    assert_eq!(after["job"]["cached"], false);
    assert_ne!(before["job"]["sha256"], after["job"]["sha256"]);
    assert_eq!(after["summary"]["pages"][0]["label_count"], 30);
    h.stop().await;
}

/* REQ-MCP-068: the cache and its jobs survive a server restart */
#[tokio::test]
async fn cache_survives_a_restart() {
    let h = Harness::start().await;
    let path = h.plan();
    let first = h.parse_small(&path).await;
    let h = h.restart().await;
    let job = h.call("get_job", json!({ "job_id": job_id(&first) })).await.ok();
    assert_eq!(job["job_id"], first["job"]["job_id"]);
    assert_eq!(h.parse_small(&path).await["job"]["cached"], true);
    h.stop().await;
}

/* REQ-MCP-069: a cache hit does not extend the life of the result */
#[tokio::test]
async fn cache_hit_keeps_the_original_expiry() {
    let h = Harness::start().await;
    let path = h.plan();
    let first = h.parse_small(&path).await;
    h.age_cache(3);
    let aged = h.call("get_job", json!({ "job_id": job_id(&first) })).await.ok();
    let hit = h.parse_small(&path).await;
    assert_eq!(hit["job"]["cached"], true);
    assert_eq!(hit["job"]["expires_at"], aged["expires_at"]);
    assert_ne!(hit["job"]["created_at"], aged["created_at"]);
    h.stop().await;
}

/* REQ-MCP-070: a result is still served on its sixth day */
#[tokio::test]
async fn result_is_cached_within_seven_days() {
    let h = Harness::start().await;
    let path = h.plan();
    h.parse_small(&path).await;
    h.age_cache(6);
    let out = h.parse_small(&path).await;
    assert_eq!(out["job"]["cached"], true);
    assert_eq!(out["job"]["cost_units"], 0.1);
    h.stop().await;
}

/* REQ-MCP-071: after seven days the result is gone and the parse is fresh */
#[tokio::test]
async fn result_expires_after_seven_days() {
    let h = Harness::start().await;
    let path = h.plan();
    h.parse_small(&path).await;
    h.age_cache(8);
    let out = h.parse_small(&path).await;
    assert_eq!(out["job"]["cached"], false);
    assert_eq!(out["job"]["cost_units"], 1.0);
    h.stop().await;
}

/* REQ-MCP-072: an expired job is not listed and cannot be read */
#[tokio::test]
async fn expired_job_is_gone() {
    let h = Harness::start().await;
    let first = h.parse_small(&h.plan()).await;
    h.age_cache(8);
    assert!(h.job_ids().await.is_empty());
    let id = job_id(&first);
    assert_eq!(h.call("get_job", json!({ "job_id": id })).await.err_code(), "not_found");
    assert_eq!(h.call("get_result", json!({ "job_id": id })).await.err_code(), "not_found");
    h.stop().await;
}

/* REQ-MCP-073: expiry removes the cached files from disk */
#[tokio::test]
async fn expiry_removes_files_from_disk() {
    let h = Harness::start().await;
    let first = h.parse_small(&h.plan()).await;
    let image = h
        .call("get_result", json!({ "job_id": job_id(&first), "mode": "page_image", "page": 0 }))
        .await
        .ok();
    let path = std::path::PathBuf::from(image["image"]["path"].as_str().unwrap());
    assert!(path.is_file());
    h.age_cache(8);
    h.call("list_jobs", json!({})).await.ok();
    assert!(!path.exists(), "expired page image is still on disk");
    h.stop().await;
}

/* REQ-MCP-074: a cache hit does not render again */
#[tokio::test]
async fn cache_hit_does_not_render_again() {
    let h = Harness::start().await;
    let path = h.plan();
    let first = h.parse_small(&path).await;
    let image = |id: String| json!({ "job_id": id, "mode": "page_image", "page": 0 });
    let before = h.call("get_result", image(job_id(&first))).await.ok();
    let file = std::path::PathBuf::from(before["image"]["path"].as_str().unwrap());
    let written = std::fs::metadata(&file).unwrap().modified().unwrap();
    let second = h.parse_small(&path).await;
    let after = h.call("get_result", image(job_id(&second))).await.ok();
    assert_eq!(after["image"]["path"], before["image"]["path"]);
    assert_eq!(std::fs::metadata(&file).unwrap().modified().unwrap(), written);
    h.stop().await;
}

/* REQ-MCP-075: after reset_file_cache the next parse is a full parse */
#[tokio::test]
async fn reset_file_cache_forces_a_fresh_parse() {
    let h = Harness::start().await;
    let path = h.plan();
    h.parse_small(&path).await;
    h.call("reset_file_cache", json!({ "path": path })).await.ok();
    let out = h.parse_small(&path).await;
    assert_eq!(out["job"]["cached"], false);
    assert_eq!(out["job"]["cost_units"], 1.0);
    h.stop().await;
}

/* REQ-MCP-076: reset_file_cache drops every size and tile variant and their jobs */
#[tokio::test]
async fn reset_file_cache_counts_what_it_removed() {
    let h = Harness::start().await;
    let path = h.plan();
    h.call("parse_file", json!({ "path": path, "size": 1024 })).await.ok();
    h.call("parse_file", json!({ "path": path, "size": 800 })).await.ok();
    h.call("parse_file", json!({ "path": path, "size": 800 })).await.ok();
    let out = h.call("reset_file_cache", json!({ "path": path })).await.ok();
    assert_eq!(out, json!({ "downloads_removed": 0, "results_removed": 2, "jobs_removed": 3 }));
    assert!(h.job_ids().await.is_empty());
    h.stop().await;
}

/* REQ-MCP-077: a job removed by reset_file_cache can no longer be read */
#[tokio::test]
async fn reset_file_cache_invalidates_job_ids() {
    let h = Harness::start().await;
    let path = h.plan();
    let id = job_id(&h.parse_small(&path).await);
    h.call("reset_file_cache", json!({ "path": path })).await.ok();
    assert_eq!(h.call("get_job", json!({ "job_id": id })).await.err_code(), "not_found");
    assert_eq!(h.call("get_result", json!({ "job_id": id })).await.err_code(), "not_found");
    h.stop().await;
}

/* REQ-MCP-078: reset_file_cache is idempotent and harmless for an unparsed file */
#[tokio::test]
async fn reset_file_cache_twice_removes_nothing() {
    let h = Harness::start().await;
    let path = h.plan();
    let nothing = json!({ "downloads_removed": 0, "results_removed": 0, "jobs_removed": 0 });
    assert_eq!(h.call("reset_file_cache", json!({ "path": path })).await.ok(), nothing);
    h.parse_small(&path).await;
    h.call("reset_file_cache", json!({ "path": path })).await.ok();
    assert_eq!(h.call("reset_file_cache", json!({ "path": path })).await.ok(), nothing);
    h.stop().await;
}

/* REQ-MCP-079: reset_file_cache leaves other drawings cached */
#[tokio::test]
async fn reset_file_cache_leaves_other_drawings() {
    let h = Harness::start().await;
    let (plan, blank) = (h.plan(), h.blank());
    h.parse_small(&plan).await;
    let kept = job_id(&h.parse_small(&blank).await);
    h.call("reset_file_cache", json!({ "path": plan })).await.ok();
    assert_eq!(h.job_ids().await, vec![kept]);
    assert_eq!(h.parse_small(&blank).await["job"]["cached"], true);
    h.stop().await;
}

/* REQ-MCP-080: reset_file_cache refuses a relative path and a missing file */
#[tokio::test]
async fn reset_file_cache_checks_the_path() {
    let h = Harness::start().await;
    let relative = h.call("reset_file_cache", json!({ "path": "plan.dwg" })).await;
    assert_eq!(relative.err_code(), "invalid_argument");
    let missing = h.call("reset_file_cache", json!({ "path": h.files_dir().join("gone.dwg") })).await;
    assert_eq!(missing.err_code(), "not_found");
    h.stop().await;
}
