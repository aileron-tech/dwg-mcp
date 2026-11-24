//! DXF input through the MCP tools.

mod common;

use common::http::{FileServer, Route};
use common::{fixtures, job_id, label_texts, Harness};
use serde_json::json;

fn plan_dxf(h: &Harness) -> std::path::PathBuf {
    fixtures::write_dxf(&fixtures::plan_document(), h.files_dir(), "plan.dxf")
}

/* REQ-MCP-200: parse_file parses a DXF into pages, labels and layers */
#[tokio::test]
async fn parse_file_reads_dxf() {
    let h = Harness::start().await;
    let out = h.parse_small(&plan_dxf(&h)).await;
    assert_eq!(out["job"]["status"], "completed");
    assert_eq!(out["summary"]["file_name"], "plan.dxf");
    assert_eq!(out["summary"]["pages"][0]["label_count"], fixtures::PLAN_LABEL_COUNT);
    let layers = common::strs(&out["summary"]["layers"]);
    assert!(layers.contains(&"WALLS".to_string()) && layers.contains(&"PIPES".to_string()));
    h.stop().await;
}

/* REQ-MCP-201: a DXF gives the same label texts and layers as the DWG of the same drawing */
#[tokio::test]
async fn dxf_and_dwg_give_the_same_labels() {
    let h = Harness::start().await;
    let from_dxf = job_id(&h.parse_small(&plan_dxf(&h)).await);
    let from_dwg = job_id(&h.parse_small(&h.plan()).await);
    let read = |id: String| json!({ "job_id": id, "mode": "labels", "page": 0 });
    let pairs = |out: &serde_json::Value| {
        let mut v: Vec<(String, String)> = out["labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| (l["text"].as_str().unwrap().to_string(), l["layer"].as_str().unwrap().to_string()))
            .collect();
        v.sort();
        v
    };
    let a = h.call("get_result", read(from_dxf)).await.ok();
    let b = h.call("get_result", read(from_dwg)).await.ok();
    assert_eq!(pairs(&a), pairs(&b));
    h.stop().await;
}

/* REQ-MCP-202: a DXF is cached like a DWG: the second parse costs a tenth */
#[tokio::test]
async fn dxf_is_cached() {
    let h = Harness::start().await;
    let path = plan_dxf(&h);
    assert_eq!(h.parse_small(&path).await["job"]["cost_units"], 1.0);
    let again = h.parse_small(&path).await;
    assert_eq!((again["job"]["cached"].clone(), again["job"]["cost_units"].clone()), (json!(true), json!(0.1)));
    h.stop().await;
}

/* REQ-MCP-203: a parsed DXF is read with get_result like any drawing */
#[tokio::test]
async fn dxf_result_is_readable() {
    let h = Harness::start().await;
    let id = job_id(&h.parse_small(&plan_dxf(&h)).await);
    let found = h
        .call("get_result", json!({ "job_id": id, "mode": "labels", "query": "dn150" }))
        .await
        .ok();
    assert_eq!(label_texts(&found), vec!["PIPE DN150"]);
    let image = h.call("get_result", json!({ "job_id": id, "mode": "page_image", "page": 0 })).await;
    assert_eq!(image.images.len(), 1);
    h.stop().await;
}

/* REQ-MCP-204: parse_url downloads and parses a DXF */
#[tokio::test]
async fn parse_url_reads_dxf() {
    let h = Harness::start().await;
    let server = FileServer::start();
    let url = server.route("/plans/plan.dxf", Route::ok("application/dxf", std::fs::read(plan_dxf(&h)).unwrap()));
    let out = h.call("parse_url", json!({ "url": url, "size": 1024 })).await.ok();
    assert_eq!(out["summary"]["file_name"], "plan.dxf");
    assert_eq!(out["summary"]["pages"][0]["label_count"], fixtures::PLAN_LABEL_COUNT);
    let again = h.call("parse_url", json!({ "url": url, "size": 1024 })).await.ok();
    assert_eq!((again["download_cached"].clone(), again["job"]["cached"].clone()), (json!(true), json!(true)));
    assert_eq!(server.hits("/plans/plan.dxf"), 1);
    h.stop().await;
}

/* REQ-MCP-205: a DXF that breaks off is a failed parse, not an empty drawing */
#[tokio::test]
async fn truncated_dxf_fails_to_parse() {
    let h = Harness::start().await;
    let whole = std::fs::read(plan_dxf(&h)).unwrap();
    // Cut in the header, in the tables, in the entities and near the end.
    for percent in [25, 50, 75, 90] {
        let broken = h.files_dir().join(format!("broken{percent}.dxf"));
        std::fs::write(&broken, &whole[..whole.len() * percent / 100]).unwrap();
        let reply = h.call("parse_file", json!({ "path": broken })).await;
        assert_eq!(reply.err_code(), "parse_failed", "cut at {percent}%: {}", reply.text);
        assert!(reply.text.contains("incomplete"), "{}", reply.text);
    }
    let listing = h.call("list_jobs", json!({})).await.ok();
    assert_eq!((listing["jobs"][0]["status"].clone(), listing["total_cost_units"].clone()), (json!("failed"), json!(0.0)));
    h.stop().await;
}

/* REQ-MCP-206: the format follows the content: a DXF named .dwg parses */
#[tokio::test]
async fn misnamed_dxf_parses() {
    let h = Harness::start().await;
    let misnamed = h.files_dir().join("export.dwg");
    std::fs::copy(plan_dxf(&h), &misnamed).unwrap();
    let out = h.parse_small(&misnamed).await;
    assert_eq!(out["summary"]["pages"][0]["label_count"], fixtures::PLAN_LABEL_COUNT);
    h.stop().await;
}

/* REQ-MCP-207: the tool definitions say that DXF is accepted */
#[tokio::test]
async fn definitions_mention_dxf() {
    let h = Harness::start().await;
    for name in ["parse_file", "parse_url"] {
        let tool = h.tool(name).await;
        assert!(tool["description"].as_str().unwrap().contains("DXF"), "{name}");
        assert!(tool["title"].as_str().unwrap().contains("DXF"), "{name}");
        assert!(tool["inputSchema"].to_string().contains(".dxf"), "{name}");
    }
    h.stop().await;
}
