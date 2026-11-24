//! parse_url and reset_url_cache: downloads, their seven-day cache, refusals.

mod common;

use common::http::{FileServer, Route};
use common::{fixtures, job_id, Harness};
use serde_json::json;

fn plan_bytes(h: &Harness) -> Vec<u8> {
    std::fs::read(h.plan()).unwrap()
}

/// A server with the plan drawing at /files/plan.dwg, and its URL.
fn serving_plan(h: &Harness) -> (FileServer, String) {
    let server = FileServer::start();
    let url = server.route("/files/plan.dwg", Route::dwg(plan_bytes(h)));
    (server, url)
}

async fn parse_url(h: &Harness, url: &str) -> serde_json::Value {
    h.call("parse_url", json!({ "url": url, "size": 1024 })).await.ok()
}

/* REQ-MCP-150: parse_url downloads the drawing and parses it as a fresh job */
#[tokio::test]
async fn parses_a_drawing_from_a_url() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    let out = parse_url(&h, &url).await;
    assert_eq!(out["job"]["status"], "completed");
    assert_eq!(out["job"]["source"], json!({ "kind": "url", "value": url }));
    assert_eq!(out["job"]["cached"], false);
    assert_eq!(out["job"]["cost_units"], 1.0);
    assert_eq!(out["download_cached"], false);
    assert_eq!(server.hits("/files/plan.dwg"), 1);
    h.stop().await;
}

/* REQ-MCP-151: parse_url gives the same summary as parse_file of the same drawing */
#[tokio::test]
async fn url_and_file_give_the_same_summary() {
    let h = Harness::start().await;
    let (_server, url) = serving_plan(&h);
    let from_url = parse_url(&h, &url).await;
    let other = Harness::start().await;
    let from_file = other.parse_small(&other.plan()).await;
    assert_eq!(from_url["summary"], from_file["summary"]);
    assert_eq!(from_url["job"]["sha256"], from_file["job"]["sha256"]);
    h.stop().await;
    other.stop().await;
}

/* REQ-MCP-152: a repeated parse_url is served from both caches at a tenth of the cost */
#[tokio::test]
async fn repeated_url_is_cached_and_cheaper() {
    let h = Harness::start().await;
    let (_server, url) = serving_plan(&h);
    parse_url(&h, &url).await;
    let again = parse_url(&h, &url).await;
    assert_eq!(again["download_cached"], true);
    assert_eq!(again["job"]["cached"], true);
    assert_eq!(again["job"]["cost_units"], 0.1);
    h.stop().await;
}

/* REQ-MCP-153: a cached download makes no second HTTP request */
#[tokio::test]
async fn cached_download_makes_no_request() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    for _ in 0..3 {
        parse_url(&h, &url).await;
    }
    assert_eq!(server.hits("/files/plan.dwg"), 1);
    h.stop().await;
}

/* REQ-MCP-154: the download cache is used even when the options differ */
#[tokio::test]
async fn download_is_reused_for_other_options() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    parse_url(&h, &url).await;
    let other = h.call("parse_url", json!({ "url": url, "size": 800, "tiles": false })).await.ok();
    assert_eq!(other["download_cached"], true);
    assert_eq!(other["job"]["cached"], false, "another size is another result");
    assert_eq!(server.hits("/files/plan.dwg"), 1);
    h.stop().await;
}

/* REQ-MCP-155: a drawing parsed from a URL is a cache hit when parsed from a file */
#[tokio::test]
async fn result_cache_is_shared_between_url_and_file() {
    let h = Harness::start().await;
    let (_server, url) = serving_plan(&h);
    parse_url(&h, &url).await;
    let from_file = h.parse_small(&h.plan()).await;
    assert_eq!(from_file["job"]["cached"], true);
    let fresh = Harness::start().await;
    let (_server2, url2) = serving_plan(&fresh);
    fresh.parse_small(&fresh.plan()).await;
    let from_url = parse_url(&fresh, &url2).await;
    assert_eq!(from_url["job"]["cached"], true);
    assert_eq!(from_url["download_cached"], false);
    h.stop().await;
    fresh.stop().await;
}

/* REQ-MCP-156: the file name comes from the URL path, percent-decoded */
#[tokio::test]
async fn file_name_comes_from_the_url_path() {
    let h = Harness::start().await;
    let server = FileServer::start();
    // The test server matches the raw request path, which is still percent-encoded.
    server.route("/a/site%20plan.dwg", Route::dwg(plan_bytes(&h)));
    let url = format!("{}/a/site%20plan.dwg?token=abc", server.base);
    let out = parse_url(&h, &url).await;
    assert_eq!(out["summary"]["file_name"], "site plan.dwg");
    h.stop().await;
}

/* REQ-MCP-157: Content-Disposition names the file when the URL does not */
#[tokio::test]
async fn file_name_comes_from_content_disposition() {
    let h = Harness::start().await;
    let server = FileServer::start();
    let route = Route::dwg(plan_bytes(&h))
        .header("Content-Disposition", "attachment; filename=\"heat-network.dwg\"");
    let url = format!("{}?id=42", server.route("/download", route));
    let out = parse_url(&h, &url).await;
    assert_eq!(out["summary"]["file_name"], "heat-network.dwg");
    assert_eq!(out["job"]["file_name"], "heat-network.dwg");
    h.stop().await;
}

/* REQ-MCP-158: redirects are followed and the final URL names the file */
#[tokio::test]
async fn redirects_are_followed() {
    let h = Harness::start().await;
    let (server, target) = serving_plan(&h);
    let url = server.route("/latest", Route::redirect(&target));
    let out = parse_url(&h, &url).await;
    assert_eq!(out["summary"]["file_name"], "plan.dwg");
    assert_eq!(out["job"]["source"]["value"], url);
    assert_eq!((server.hits("/latest"), server.hits("/files/plan.dwg")), (1, 1));
    h.stop().await;
}

/* REQ-MCP-159: an HTTP error status is download_failed and names the status */
#[tokio::test]
async fn http_errors_are_download_failed() {
    let h = Harness::start().await;
    let server = FileServer::start();
    for status in [404, 403, 500] {
        let url = server.route(&format!("/s{status}/plan.dwg"), Route::status(status));
        let reply = h.call("parse_url", json!({ "url": url })).await;
        assert_eq!(reply.err_code(), "download_failed", "status {status}");
        assert!(reply.text.contains(&status.to_string()), "{}", reply.text);
    }
    assert!(h.job_ids().await.is_empty());
    h.stop().await;
}

/* REQ-MCP-160: a host that does not answer is download_failed */
#[tokio::test]
async fn unreachable_host_is_download_failed() {
    let h = Harness::start().await;
    let closed = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}/plan.dwg", listener.local_addr().unwrap())
    };
    let reply = h.call("parse_url", json!({ "url": closed })).await;
    assert_eq!(reply.err_code(), "download_failed");
    h.stop().await;
}

/* REQ-MCP-161: a web page is refused as unsupported_file */
#[tokio::test]
async fn web_page_is_unsupported() {
    let h = Harness::start().await;
    let server = FileServer::start();
    let page = Route::ok("text/html; charset=utf-8", b"<html>Sign in</html>".to_vec());
    let url = server.route("/share/plan.dwg", page);
    let reply = h.call("parse_url", json!({ "url": url })).await;
    assert_eq!(reply.err_code(), "unsupported_file");
    assert!(reply.text.contains("web page"));
    h.stop().await;
}

/* REQ-MCP-162: a file that is not named .dwg is refused */
#[tokio::test]
async fn other_file_type_is_unsupported() {
    let h = Harness::start().await;
    let server = FileServer::start();
    for name in ["/plan.dxf", "/plan.pdf", "/plan"] {
        let url = server.route(name, Route::dwg(plan_bytes(&h)));
        let reply = h.call("parse_url", json!({ "url": url })).await;
        assert_eq!(reply.err_code(), "unsupported_file", "{name}");
    }
    h.stop().await;
}

/* REQ-MCP-163: a .dwg download without a DWG version code is refused and not cached */
#[tokio::test]
async fn non_dwg_download_is_refused_and_not_cached() {
    let h = Harness::start().await;
    let server = FileServer::start();
    let url = server.route("/plan.dwg", Route::dwg(b"this is not a drawing".to_vec()));
    for _ in 0..2 {
        let reply = h.call("parse_url", json!({ "url": url })).await;
        assert_eq!(reply.err_code(), "unsupported_file");
    }
    assert_eq!(server.hits("/plan.dwg"), 2, "a refused download must not be cached");
    h.stop().await;
}

/* REQ-MCP-164: a download announced as larger than 100 MB is refused as too_large */
#[tokio::test]
async fn oversized_download_is_too_large() {
    let h = Harness::start().await;
    let server = FileServer::start();
    let route = Route::dwg(plan_bytes(&h)).declared_len(100 * 1024 * 1024 + 1);
    let url = server.route("/huge.dwg", route);
    let reply = h.call("parse_url", json!({ "url": url })).await;
    assert_eq!(reply.err_code(), "too_large");
    h.stop().await;
}

/* REQ-MCP-165: only http and https URLs with a host are accepted */
#[tokio::test]
async fn url_scheme_is_checked() {
    let h = Harness::start().await;
    for url in ["ftp://example.com/plan.dwg", "file:///tmp/plan.dwg", "plan.dwg", "http:///plan.dwg", ""] {
        let reply = h.call("parse_url", json!({ "url": url })).await;
        assert_eq!(reply.err_code(), "invalid_argument", "url {url:?}");
    }
    h.stop().await;
}

/* REQ-MCP-166: parse_url checks size before it downloads anything */
#[tokio::test]
async fn options_are_checked_before_the_download() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    let reply = h.call("parse_url", json!({ "url": url, "size": 100 })).await;
    assert_eq!(reply.err_code(), "invalid_argument");
    assert_eq!(server.hits("/files/plan.dwg"), 0);
    h.stop().await;
}

/* REQ-MCP-167: URLs that differ in their query string are separate downloads of one result */
#[tokio::test]
async fn query_string_is_part_of_the_download_key() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    let first = parse_url(&h, &format!("{url}?v=1")).await;
    let second = parse_url(&h, &format!("{url}?v=2")).await;
    assert_eq!(second["download_cached"], false);
    assert_eq!(second["job"]["cached"], true, "same content, so the result is shared");
    assert_eq!(first["job"]["sha256"], second["job"]["sha256"]);
    assert_eq!(server.hits("/files/plan.dwg"), 2);
    h.stop().await;
}

/* REQ-MCP-168: a download is still cached on its sixth day */
#[tokio::test]
async fn download_is_cached_within_seven_days() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    parse_url(&h, &url).await;
    h.age_cache(6);
    let again = parse_url(&h, &url).await;
    assert_eq!(again["download_cached"], true);
    assert_eq!(server.hits("/files/plan.dwg"), 1);
    h.stop().await;
}

/* REQ-MCP-169: after seven days the download is fetched again and parsed fresh */
#[tokio::test]
async fn download_expires_after_seven_days() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    parse_url(&h, &url).await;
    h.age_cache(8);
    let again = parse_url(&h, &url).await;
    assert_eq!(again["download_cached"], false);
    assert_eq!(again["job"]["cached"], false);
    assert_eq!(again["job"]["cost_units"], 1.0);
    assert_eq!(server.hits("/files/plan.dwg"), 2);
    h.stop().await;
}

/* REQ-MCP-170: within seven days a changed remote file is not seen without a reset */
#[tokio::test]
async fn changed_remote_file_is_not_seen_while_cached() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    let first = parse_url(&h, &url).await;
    let changed = fixtures::write_dwg(&fixtures::dense_document(40), h.files_dir(), "new.dwg");
    server.route("/files/plan.dwg", Route::dwg(std::fs::read(changed).unwrap()));
    let again = parse_url(&h, &url).await;
    assert_eq!(again["download_cached"], true);
    assert_eq!(again["job"]["sha256"], first["job"]["sha256"]);
    h.stop().await;
}

/* REQ-MCP-171: reset_url_cache makes the next parse_url fetch and parse the changed file */
#[tokio::test]
async fn reset_url_cache_picks_up_a_changed_file() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    let first = parse_url(&h, &url).await;
    let changed = fixtures::write_dwg(&fixtures::dense_document(40), h.files_dir(), "new.dwg");
    server.route("/files/plan.dwg", Route::dwg(std::fs::read(changed).unwrap()));
    h.call("reset_url_cache", json!({ "url": url })).await.ok();
    let again = parse_url(&h, &url).await;
    assert_eq!(again["download_cached"], false);
    assert_eq!(again["job"]["cached"], false);
    assert_ne!(again["job"]["sha256"], first["job"]["sha256"]);
    assert_eq!(again["summary"]["pages"][0]["label_count"], 40);
    assert_eq!(server.hits("/files/plan.dwg"), 2);
    h.stop().await;
}

/* REQ-MCP-172: reset_url_cache reports the download, results and jobs it removed */
#[tokio::test]
async fn reset_url_cache_counts_what_it_removed() {
    let h = Harness::start().await;
    let (_server, url) = serving_plan(&h);
    let id = job_id(&parse_url(&h, &url).await);
    parse_url(&h, &url).await;
    h.call("parse_url", json!({ "url": url, "size": 800 })).await.ok();
    let out = h.call("reset_url_cache", json!({ "url": url })).await.ok();
    assert_eq!(out, json!({ "downloads_removed": 1, "results_removed": 2, "jobs_removed": 3 }));
    assert_eq!(h.call("get_job", json!({ "job_id": id })).await.err_code(), "not_found");
    h.stop().await;
}

/* REQ-MCP-173: reset_url_cache is idempotent and makes no network request */
#[tokio::test]
async fn reset_url_cache_twice_removes_nothing() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    let nothing = json!({ "downloads_removed": 0, "results_removed": 0, "jobs_removed": 0 });
    assert_eq!(h.call("reset_url_cache", json!({ "url": url })).await.ok(), nothing);
    parse_url(&h, &url).await;
    h.call("reset_url_cache", json!({ "url": url })).await.ok();
    assert_eq!(h.call("reset_url_cache", json!({ "url": url })).await.ok(), nothing);
    assert_eq!(server.hits("/files/plan.dwg"), 1, "reset must not fetch");
    h.stop().await;
}

/* REQ-MCP-174: reset_url_cache refuses a string that is not an http(s) URL */
#[tokio::test]
async fn reset_url_cache_checks_the_url() {
    let h = Harness::start().await;
    let reply = h.call("reset_url_cache", json!({ "url": "/local/plan.dwg" })).await;
    assert_eq!(reply.err_code(), "invalid_argument");
    h.stop().await;
}

/* REQ-MCP-175: reset_url_cache leaves other URLs cached */
#[tokio::test]
async fn reset_url_cache_leaves_other_urls() {
    let h = Harness::start().await;
    let server = FileServer::start();
    let plan = server.route("/plan.dwg", Route::dwg(plan_bytes(&h)));
    let blank = server.route("/blank.dwg", Route::dwg(std::fs::read(h.blank()).unwrap()));
    parse_url(&h, &plan).await;
    parse_url(&h, &blank).await;
    h.call("reset_url_cache", json!({ "url": plan })).await.ok();
    let kept = parse_url(&h, &blank).await;
    assert_eq!((kept["download_cached"].clone(), kept["job"]["cached"].clone()), (json!(true), json!(true)));
    assert_eq!(server.hits("/blank.dwg"), 1);
    h.stop().await;
}

/* REQ-MCP-176: reset_file_cache drops the result but keeps the download of the same drawing */
#[tokio::test]
async fn reset_file_cache_keeps_the_download() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    parse_url(&h, &url).await;
    let out = h.call("reset_file_cache", json!({ "path": h.plan() })).await.ok();
    assert_eq!(out, json!({ "downloads_removed": 0, "results_removed": 1, "jobs_removed": 1 }));
    let again = parse_url(&h, &url).await;
    assert_eq!(again["download_cached"], true);
    assert_eq!(again["job"]["cached"], false);
    assert_eq!(server.hits("/files/plan.dwg"), 1);
    h.stop().await;
}

/* REQ-MCP-177: the downloads and their jobs survive a server restart */
#[tokio::test]
async fn download_cache_survives_a_restart() {
    let h = Harness::start().await;
    let (server, url) = serving_plan(&h);
    parse_url(&h, &url).await;
    let h = h.restart().await;
    let again = parse_url(&h, &url).await;
    assert_eq!(again["download_cached"], true);
    assert_eq!(again["job"]["cached"], true);
    assert_eq!(server.hits("/files/plan.dwg"), 1);
    h.stop().await;
}
