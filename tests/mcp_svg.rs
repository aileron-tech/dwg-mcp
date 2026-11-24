//! SVG drawings through the MCP stdio client.
mod common;

use common::http::{FileServer, Route};
use common::{job_id, label_texts, Harness};
use serde_json::json;

const PLAN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100">
    <path d="M 5 5 H 195 V 95 H 5 Z" fill="none" stroke="black"/>
    <text x="20" y="40" font-size="12">Room 101</text>
</svg>"#;

/* REQ-MCP-208: parse_file returns a single SVG page and its native labels */
#[tokio::test]
async fn parse_file_reads_svg() {
    let h = Harness::start().await;
    let path = h.files_dir().join("plan.SVG");
    std::fs::write(&path, PLAN).unwrap();
    let parsed = h.parse_small(&path).await;
    assert_eq!(parsed["summary"]["page_count"], 1);
    assert_eq!(parsed["summary"]["pages"][0]["label_count"], 1);
    let labels = h
        .call("get_result", json!({ "job_id": job_id(&parsed), "mode": "labels" }))
        .await
        .ok();
    assert_eq!(label_texts(&labels), ["Room 101"]);
    h.stop().await;
}

/* REQ-MCP-209: parse_url reads and caches SVG drawings */
#[tokio::test]
async fn parse_url_reads_svg() {
    let h = Harness::start().await;
    let server = FileServer::start();
    let url = server.route("/plan.svg", Route::ok("image/svg+xml", PLAN.as_bytes().to_vec()));
    let input = json!({ "url": url, "size": 512 });
    let parsed = h.call("parse_url", input.clone()).await.ok();
    assert_eq!(parsed["summary"]["file_name"], "plan.svg");
    let again = h.call("parse_url", input).await.ok();
    assert_eq!(again["download_cached"], true);
    assert_eq!(again["job"]["cached"], true);
    assert_eq!(server.hits("/plan.svg"), 1);
    h.stop().await;
}

/* REQ-MCP-210: an SVG root with truncated XML fails without charging */
#[tokio::test]
async fn truncated_svg_is_parse_failed() {
    let h = Harness::start().await;
    let path = h.files_dir().join("broken.svg");
    std::fs::write(&path, PLAN.strip_suffix("</svg>").unwrap()).unwrap();
    let reply = h.call("parse_file", json!({ "path": path })).await;
    assert_eq!(reply.err_code(), "parse_failed");
    let jobs = h.call("list_jobs", json!({})).await.ok();
    assert_eq!(jobs["total_cost_units"], 0.0);
    h.stop().await;
}

/* REQ-MCP-211: page_svg exposes a vector export and leaves page_image available for all inputs */
#[tokio::test]
async fn vector_output_is_available_for_all_formats() {
    let h = Harness::start().await;
    let svg = h.files_dir().join("plan.svg");
    std::fs::write(&svg, PLAN).unwrap();
    let dxf = common::fixtures::write_dxf(&common::fixtures::plan_document(), h.files_dir(), "plan.dxf");
    for path in [svg, dxf, h.plan()] {
        let parsed = h.parse_small(&path).await;
        for page in 0..parsed["summary"]["page_count"].as_u64().unwrap() {
            let id = job_id(&parsed);
            let vector = h.call("get_result", json!({ "job_id": id, "mode": "page_svg", "page": page })).await;
            assert!(vector.images.is_empty());
            let vector = vector.ok();
            let image = &vector["image"];
            assert_eq!(image["mime_type"], "image/svg+xml");
            assert_eq!(image["inline"], false);
            let svg = std::fs::read_to_string(image["path"].as_str().unwrap()).unwrap();
            assert!(svg.starts_with("<svg") && !svg.contains("<image"));
            let png = h.call("get_result", json!({ "job_id": id, "mode": "page_image", "page": page })).await;
            assert_eq!(png.images.len(), 1);
        }
    }
    h.stop().await;
}
