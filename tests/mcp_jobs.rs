//! get_job, list_jobs and delete_job.

mod common;

use common::{job_id, Harness};
use serde_json::json;

/* REQ-MCP-130: get_job returns the job that the parse returned */
#[tokio::test]
async fn get_job_returns_the_parse_job() {
    let h = Harness::start().await;
    let out = h.parse_small(&h.plan()).await;
    let job = h.call("get_job", json!({ "job_id": job_id(&out) })).await.ok();
    assert_eq!(job, out["job"]);
    h.stop().await;
}

/* REQ-MCP-131: get_job with an unknown or malformed id is not_found */
#[tokio::test]
async fn get_job_unknown_id_is_not_found() {
    let h = Harness::start().await;
    h.parse_small(&h.blank()).await;
    for id in ["job_ffffffffffffffff", "", "../jobs/x", "job_1/../../etc"] {
        let reply = h.call("get_job", json!({ "job_id": id })).await;
        assert_eq!(reply.err_code(), "not_found", "id {id:?}");
    }
    h.stop().await;
}

/* REQ-MCP-132: list_jobs on an empty cache is an empty list at zero cost */
#[tokio::test]
async fn list_jobs_starts_empty() {
    let h = Harness::start().await;
    let listing = h.call("list_jobs", json!({})).await.ok();
    assert_eq!(listing, json!({ "jobs": [], "total": 0, "total_cost_units": 0.0 }));
    h.stop().await;
}

/* REQ-MCP-133: list_jobs returns the newest job first */
#[tokio::test]
async fn list_jobs_is_newest_first() {
    let h = Harness::start().await;
    let (plan, blank) = (h.plan(), h.blank());
    let first = job_id(&h.parse_small(&plan).await);
    let second = job_id(&h.parse_small(&blank).await);
    let third = job_id(&h.parse_small(&plan).await);
    assert_eq!(h.job_ids().await, vec![third, second, first]);
    h.stop().await;
}

/* REQ-MCP-134: list_jobs totals the cost: a fresh parse plus a cache hit is 1.1 */
#[tokio::test]
async fn list_jobs_totals_the_cost() {
    let h = Harness::start().await;
    let path = h.plan();
    h.parse_small(&path).await;
    h.parse_small(&path).await;
    let listing = h.call("list_jobs", json!({})).await.ok();
    assert_eq!(listing["total"], 2);
    assert_eq!(listing["total_cost_units"], 1.1);
    h.parse_small(&h.blank()).await;
    assert_eq!(h.call("list_jobs", json!({})).await.ok()["total_cost_units"], 2.1);
    h.stop().await;
}

/* REQ-MCP-135: list_jobs pages with limit and cursor, and the total covers all pages */
#[tokio::test]
async fn list_jobs_pages_with_a_cursor() {
    let h = Harness::start().await;
    let path = h.blank();
    for _ in 0..5 {
        h.call("parse_file", json!({ "path": path, "size": 512 })).await.ok();
    }
    let all = h.job_ids().await;
    let first = h.call("list_jobs", json!({ "limit": 2 })).await.ok();
    assert_eq!(first["jobs"].as_array().unwrap().len(), 2);
    assert_eq!(first["total"], 5);
    assert_eq!(first["total_cost_units"], 1.4);
    let second = h.call("list_jobs", json!({ "limit": 2, "cursor": first["next_cursor"] })).await.ok();
    let third = h.call("list_jobs", json!({ "limit": 2, "cursor": second["next_cursor"] })).await.ok();
    assert!(third.get("next_cursor").is_none());
    let mut walked = Vec::new();
    for page in [&first, &second, &third] {
        walked.extend(page["jobs"].as_array().unwrap().iter().map(|j| j["job_id"].as_str().unwrap().to_string()));
    }
    assert_eq!(walked, all);
    h.stop().await;
}

/* REQ-MCP-136: list_jobs refuses a limit outside 1..100 and a foreign cursor */
#[tokio::test]
async fn list_jobs_checks_limit_and_cursor() {
    let h = Harness::start().await;
    for limit in [0, 101] {
        let reply = h.call("list_jobs", json!({ "limit": limit })).await;
        assert_eq!(reply.err_code(), "invalid_argument", "limit {limit}");
    }
    assert_eq!(h.call("list_jobs", json!({ "cursor": "next" })).await.err_code(), "invalid_argument");
    assert_eq!(h.call("list_jobs", json!({ "limit": 100 })).await.ok()["total"], 0);
    h.stop().await;
}

/* REQ-MCP-137: delete_job removes the job and, when it was the last reader, the result */
#[tokio::test]
async fn delete_job_removes_job_and_result() {
    let h = Harness::start().await;
    let path = h.plan();
    let id = job_id(&h.parse_small(&path).await);
    let out = h.call("delete_job", json!({ "job_id": id })).await.ok();
    assert_eq!(out, json!({ "job_id": id, "deleted": true, "result_removed": true }));
    assert_eq!(h.call("get_job", json!({ "job_id": id })).await.err_code(), "not_found");
    assert_eq!(h.call("get_result", json!({ "job_id": id })).await.err_code(), "not_found");
    assert_eq!(h.parse_small(&path).await["job"]["cached"], false, "the result must be gone");
    h.stop().await;
}

/* REQ-MCP-138: delete_job keeps a result that another job still reads */
#[tokio::test]
async fn delete_job_keeps_a_shared_result() {
    let h = Harness::start().await;
    let path = h.plan();
    let first = job_id(&h.parse_small(&path).await);
    let second = job_id(&h.parse_small(&path).await);
    let out = h.call("delete_job", json!({ "job_id": first })).await.ok();
    assert_eq!(out["result_removed"], false);
    let read = h.call("get_result", json!({ "job_id": second, "mode": "labels" })).await.ok();
    assert!(read["total"].as_u64().unwrap() > 0);
    assert_eq!(h.parse_small(&path).await["job"]["cached"], true);
    h.stop().await;
}

/* REQ-MCP-139: deleting the last reader of a shared result removes it */
#[tokio::test]
async fn deleting_every_reader_removes_the_result() {
    let h = Harness::start().await;
    let path = h.plan();
    let first = job_id(&h.parse_small(&path).await);
    let second = job_id(&h.parse_small(&path).await);
    assert_eq!(h.call("delete_job", json!({ "job_id": first })).await.ok()["result_removed"], false);
    assert_eq!(h.call("delete_job", json!({ "job_id": second })).await.ok()["result_removed"], true);
    assert!(h.job_ids().await.is_empty());
    h.stop().await;
}

/* REQ-MCP-140: delete_job on an unknown or already deleted job is not_found */
#[tokio::test]
async fn delete_job_twice_is_not_found() {
    let h = Harness::start().await;
    let id = job_id(&h.parse_small(&h.blank()).await);
    assert_eq!(h.call("delete_job", json!({ "job_id": "job_0123456789abcdef" })).await.err_code(), "not_found");
    h.call("delete_job", json!({ "job_id": id })).await.ok();
    assert_eq!(h.call("delete_job", json!({ "job_id": id })).await.err_code(), "not_found");
    h.stop().await;
}

/* REQ-MCP-141: delete_job leaves the other jobs and their results alone */
#[tokio::test]
async fn delete_job_leaves_other_jobs() {
    let h = Harness::start().await;
    let gone = job_id(&h.parse_small(&h.plan()).await);
    let kept = job_id(&h.parse_small(&h.blank()).await);
    h.call("delete_job", json!({ "job_id": gone })).await.ok();
    assert_eq!(h.job_ids().await, vec![kept.clone()]);
    h.call("get_result", json!({ "job_id": kept, "mode": "page_image", "page": 0 })).await.ok();
    h.stop().await;
}

/* REQ-MCP-142: a failed job can be deleted; it has no result to remove */
#[tokio::test]
async fn failed_job_can_be_deleted() {
    let h = Harness::start().await;
    let whole = std::fs::read(h.plan()).unwrap();
    let damaged = h.files_dir().join("damaged.dwg");
    std::fs::write(&damaged, &whole[..1000]).unwrap();
    assert!(h.call("parse_file", json!({ "path": damaged })).await.is_error);
    let id = h.job_ids().await.remove(0);
    let out = h.call("delete_job", json!({ "job_id": id })).await.ok();
    assert_eq!(out["result_removed"], false);
    assert!(h.job_ids().await.is_empty());
    h.stop().await;
}
