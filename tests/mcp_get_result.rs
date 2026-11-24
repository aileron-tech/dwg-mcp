//! get_result: the five modes, label filters and paging, images.

mod common;

use std::collections::BTreeSet;

use common::{fixtures, job_id, label_texts, png_size, strs, Harness};
use serde_json::{json, Value};

async fn parsed_plan(h: &Harness) -> (String, Value) {
    let out = h.parse_small(&h.plan()).await;
    (job_id(&out), out["summary"].clone())
}

/* REQ-MCP-090: without a mode, get_result returns the summary of the parse */
#[tokio::test]
async fn default_mode_is_the_summary() {
    let h = Harness::start().await;
    let (id, summary) = parsed_plan(&h).await;
    let out = h.call("get_result", json!({ "job_id": id })).await.ok();
    assert_eq!(out["mode"], "summary");
    assert_eq!(out["summary"], summary);
    assert_eq!(out["job_id"], id);
    for absent in ["index", "labels", "image", "total", "next_cursor"] {
        assert!(out.get(absent).is_none(), "{absent} does not belong to mode summary");
    }
    h.stop().await;
}

/* REQ-MCP-091: an unknown job_id is not_found and points to list_jobs */
#[tokio::test]
async fn unknown_job_is_not_found() {
    let h = Harness::start().await;
    let reply = h.call("get_result", json!({ "job_id": "job_0000000000000000" })).await;
    assert_eq!(reply.err_code(), "not_found");
    assert!(reply.text.contains("list_jobs"));
    h.stop().await;
}

/* REQ-MCP-092: mode index returns layers, layouts and the entity histogram */
#[tokio::test]
async fn index_lists_layers_layouts_and_entity_counts() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    let index = h.call("get_result", json!({ "job_id": id, "mode": "index" })).await.ok()["index"].clone();
    assert!(strs(&index["layers"]).contains(&"WALLS".to_string()));
    let layouts: Vec<&str> =
        index["layouts"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
    assert!(layouts.contains(&"Model") && layouts.contains(&"Layout1"));
    let count = |kind: &str| {
        index["histogram"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["kind"] == kind)
            .map(|e| e["count"].as_u64().unwrap())
    };
    assert_eq!((count("Line"), count("Text"), count("Circle"), count("MText")), (Some(5), Some(5), Some(1), Some(1)));
    assert_eq!(index["file_name"], "plan.dwg");
    h.stop().await;
}

/* REQ-MCP-093: the index carries a sample of the drawing's text */
#[tokio::test]
async fn index_samples_text() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    let index = h.call("get_result", json!({ "job_id": id, "mode": "index" })).await.ok()["index"].clone();
    let samples = strs(&index["text_samples"]);
    assert!(samples.contains(&fixtures::PLAN_PIPE_LABELS[0].to_string()));
    h.stop().await;
}

/* REQ-MCP-094: mode labels returns every label of the drawing with a total */
#[tokio::test]
async fn labels_returns_all_labels() {
    let h = Harness::start().await;
    let (id, summary) = parsed_plan(&h).await;
    let out = h.call("get_result", json!({ "job_id": id, "mode": "labels" })).await.ok();
    assert_eq!(out["total"], summary["label_count"]);
    assert_eq!(out["labels"].as_array().unwrap().len() as u64, summary["label_count"].as_u64().unwrap());
    assert!(out.get("next_cursor").is_none());
    h.stop().await;
}

/* REQ-MCP-095: labels carry the exact text written in the drawing */
#[tokio::test]
async fn labels_are_the_exact_text_of_the_drawing() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    let out = h.call("get_result", json!({ "job_id": id, "mode": "labels", "page": 0 })).await.ok();
    let got: BTreeSet<String> = label_texts(&out).into_iter().collect();
    let mut expected: BTreeSet<String> = BTreeSet::new();
    expected.extend(fixtures::PLAN_WALL_LABELS.iter().map(|s| s.to_string()));
    expected.extend(fixtures::PLAN_PIPE_LABELS.iter().map(|s| s.to_string()));
    expected.insert(fixtures::PLAN_MTEXT_LABEL.to_string());
    assert_eq!(got, expected);
    h.stop().await;
}

/* REQ-MCP-096: the page filter keeps the labels of one page */
#[tokio::test]
async fn labels_filter_by_page() {
    let h = Harness::start().await;
    let (id, summary) = parsed_plan(&h).await;
    let out = h.call("get_result", json!({ "job_id": id, "mode": "labels", "page": 1 })).await.ok();
    assert_eq!(out["total"], summary["pages"][1]["label_count"]);
    assert!(out["labels"].as_array().unwrap().iter().all(|l| l["page"] == 1));
    h.stop().await;
}

/* REQ-MCP-097: the layer filter matches the layer name without regard to case */
#[tokio::test]
async fn labels_filter_by_layer() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    for layer in ["PIPES", "pipes"] {
        let out = h
            .call("get_result", json!({ "job_id": id, "mode": "labels", "page": 0, "layer": layer }))
            .await
            .ok();
        let mut texts = label_texts(&out);
        texts.sort();
        assert_eq!(texts, fixtures::PLAN_PIPE_LABELS.to_vec(), "layer {layer}");
        assert!(out["labels"].as_array().unwrap().iter().all(|l| l["layer"] == "PIPES"));
    }
    h.stop().await;
}

/* REQ-MCP-098: the query filter is a case-insensitive substring match */
#[tokio::test]
async fn labels_filter_by_query() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    let out = h
        .call("get_result", json!({ "job_id": id, "mode": "labels", "page": 0, "query": "wall a-" }))
        .await
        .ok();
    let mut texts = label_texts(&out);
    texts.sort();
    assert_eq!(texts, vec!["WALL A-101", "WALL A-102"]);
    h.stop().await;
}

/* REQ-MCP-099: page, layer and query filters combine */
#[tokio::test]
async fn label_filters_combine() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    let args = json!({ "job_id": id, "mode": "labels", "page": 0, "layer": "pipes", "query": "dn150" });
    let out = h.call("get_result", args).await.ok();
    assert_eq!(label_texts(&out), vec!["PIPE DN150"]);
    assert_eq!(out["total"], 1);
    h.stop().await;
}

/* REQ-MCP-100: a filter that matches nothing returns an empty list, not an error */
#[tokio::test]
async fn labels_with_no_match_are_empty() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    let out = h
        .call("get_result", json!({ "job_id": id, "mode": "labels", "query": "no such text" }))
        .await
        .ok();
    assert_eq!(out["total"], 0);
    assert_eq!(out["labels"], json!([]));
    let unknown_layer =
        h.call("get_result", json!({ "job_id": id, "mode": "labels", "layer": "ROOF" })).await.ok();
    assert_eq!(unknown_layer["total"], 0);
    h.stop().await;
}

/* REQ-MCP-101: label boxes lie inside their page image and have a size */
#[tokio::test]
async fn label_boxes_lie_inside_the_page() {
    let h = Harness::start().await;
    let (id, summary) = parsed_plan(&h).await;
    let out = h.call("get_result", json!({ "job_id": id, "mode": "labels" })).await.ok();
    for label in out["labels"].as_array().unwrap() {
        let page = &summary["pages"][label["page"].as_u64().unwrap() as usize];
        let (x, y) = (label["x"].as_f64().unwrap(), label["y"].as_f64().unwrap());
        assert!(x >= 0.0 && x <= page["width"].as_f64().unwrap(), "{label}");
        assert!(y >= 0.0 && y <= page["height"].as_f64().unwrap(), "{label}");
        assert!(label["w"].as_f64().unwrap() > 0.0 && label["h"].as_f64().unwrap() > 0.0);
    }
    h.stop().await;
}

/* REQ-MCP-102: a label carries its layer and the layer's color */
#[tokio::test]
async fn labels_carry_layer_and_color() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    let out = h
        .call("get_result", json!({ "job_id": id, "mode": "labels", "page": 0, "query": "WALL A-101" }))
        .await
        .ok();
    let label = &out["labels"][0];
    assert_eq!(label["layer"], "WALLS");
    assert_eq!(label["color"], "#ff0000");
    h.stop().await;
}

/* REQ-MCP-103: labels are paged, 100 per page by default */
#[tokio::test]
async fn labels_page_at_100_by_default() {
    let h = Harness::start().await;
    let id = job_id(&h.parse_small(&h.dense(250)).await);
    let out = h.call("get_result", json!({ "job_id": id, "mode": "labels" })).await.ok();
    assert_eq!(out["labels"].as_array().unwrap().len(), 100);
    assert_eq!(out["total"], 250);
    assert_eq!(out["next_cursor"], "100");
    h.stop().await;
}

/* REQ-MCP-104: following next_cursor visits every label exactly once */
#[tokio::test]
async fn cursor_walks_every_label_once() {
    let h = Harness::start().await;
    let id = job_id(&h.parse_small(&h.dense(250)).await);
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    let mut calls = 0;
    loop {
        let mut args = json!({ "job_id": id, "mode": "labels", "limit": 60 });
        if let Some(c) = &cursor {
            args["cursor"] = json!(c);
        }
        let out = h.call("get_result", args).await.ok();
        seen.extend(label_texts(&out));
        calls += 1;
        cursor = out.get("next_cursor").and_then(Value::as_str).map(str::to_string);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(calls, 5);
    assert_eq!(seen.len(), 250);
    assert_eq!(seen.iter().collect::<BTreeSet<_>>().len(), 250);
    h.stop().await;
}

/* REQ-MCP-105: limit accepts 1 to 500 and nothing else */
#[tokio::test]
async fn label_limit_is_bounded() {
    let h = Harness::start().await;
    let id = job_id(&h.parse_small(&h.dense(250)).await);
    let all = h.call("get_result", json!({ "job_id": id, "mode": "labels", "limit": 500 })).await.ok();
    assert_eq!(all["labels"].as_array().unwrap().len(), 250);
    let one = h.call("get_result", json!({ "job_id": id, "mode": "labels", "limit": 1 })).await.ok();
    assert_eq!(one["labels"].as_array().unwrap().len(), 1);
    for limit in [0, 501] {
        let reply = h.call("get_result", json!({ "job_id": id, "mode": "labels", "limit": limit })).await;
        assert_eq!(reply.err_code(), "invalid_argument", "limit {limit}");
    }
    h.stop().await;
}

/* REQ-MCP-106: a cursor that this tool did not issue is refused; one past the end is empty */
#[tokio::test]
async fn label_cursor_is_checked() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    for cursor in ["abc", "-5", ""] {
        let reply = h.call("get_result", json!({ "job_id": id, "mode": "labels", "cursor": cursor })).await;
        assert_eq!(reply.err_code(), "invalid_argument", "cursor {cursor:?}");
    }
    let past = h.call("get_result", json!({ "job_id": id, "mode": "labels", "cursor": "9999" })).await.ok();
    assert_eq!(past["labels"], json!([]));
    assert!(past.get("next_cursor").is_none());
    h.stop().await;
}

/* REQ-MCP-107: a page that does not exist is refused with the valid range */
#[tokio::test]
async fn page_out_of_range_is_refused() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    for mode in ["labels", "page_image", "tile_image"] {
        let reply = h.call("get_result", json!({ "job_id": id, "mode": mode, "page": 2, "row": 0, "col": 0 })).await;
        assert_eq!(reply.err_code(), "invalid_argument", "mode {mode}");
        assert!(reply.text.contains("0 to 1"), "{}", reply.text);
    }
    h.stop().await;
}

/* REQ-MCP-108: page_image returns the page as PNG image content of the reported size */
#[tokio::test]
async fn page_image_returns_a_png() {
    let h = Harness::start().await;
    let (id, summary) = parsed_plan(&h).await;
    let reply = h.call("get_result", json!({ "job_id": id, "mode": "page_image", "page": 0 })).await;
    assert_eq!(reply.images.len(), 1);
    let (mime, png) = &reply.images[0];
    assert_eq!(mime, "image/png");
    let page = &summary["pages"][0];
    assert_eq!(png_size(png), (page["width"].as_u64().unwrap() as u32, page["height"].as_u64().unwrap() as u32));
    let image = &reply.data["image"];
    assert_eq!(image["inline"], true);
    assert_eq!(image["mime_type"], "image/png");
    assert_eq!((image["width"].clone(), image["height"].clone()), (page["width"].clone(), page["height"].clone()));
    h.stop().await;
}

/* REQ-MCP-109: the image path points at the same PNG that was returned inline */
#[tokio::test]
async fn image_path_holds_the_same_png() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    let reply = h.call("get_result", json!({ "job_id": id, "mode": "page_image", "page": 1 })).await;
    let image = &reply.data["image"];
    let on_disk = std::fs::read(image["path"].as_str().unwrap()).unwrap();
    assert_eq!(on_disk, reply.images[0].1);
    assert_eq!(image["bytes"].as_u64().unwrap() as usize, on_disk.len());
    assert_eq!(image["page"], 1);
    assert!(image.get("row").is_none() && image.get("col").is_none());
    h.stop().await;
}

/* REQ-MCP-110: page_image and tile_image require a page */
#[tokio::test]
async fn image_modes_require_a_page() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    for mode in ["page_image", "tile_image"] {
        let reply = h.call("get_result", json!({ "job_id": id, "mode": mode })).await;
        assert_eq!(reply.err_code(), "invalid_argument", "mode {mode}");
        assert!(reply.text.contains("page"));
    }
    h.stop().await;
}

/* REQ-MCP-111: tile_image returns one tile as a PNG with the labels inside it */
#[tokio::test]
async fn tile_image_returns_a_tile_and_its_labels() {
    let h = Harness::start().await;
    let out = h.parse_file(&h.plan()).await;
    let args = json!({ "job_id": job_id(&out), "mode": "tile_image", "page": 0, "row": 0, "col": 0 });
    let reply = h.call("get_result", args).await;
    let image = &reply.data["image"];
    let size = (image["width"].as_u64().unwrap() as u32, image["height"].as_u64().unwrap() as u32);
    assert_eq!(png_size(&reply.images[0].1), size);
    assert_eq!((image["row"].clone(), image["col"].clone()), (json!(0), json!(0)));
    assert_eq!((image["x"].clone(), image["y"].clone()), (json!(0), json!(0)));
    let texts = label_texts(&reply.data);
    assert!(texts.contains(&"WALL A-101".to_string()), "{texts:?}");
    for label in reply.data["labels"].as_array().unwrap() {
        assert!(label["x"].as_f64().unwrap() <= f64::from(size.0), "tile labels use tile pixels");
    }
    h.stop().await;
}

/* REQ-MCP-112: tiles are offset in the page by their column and row */
#[tokio::test]
async fn tiles_are_offset_within_the_page() {
    let h = Harness::start().await;
    let out = h.parse_file(&h.plan()).await;
    let id = job_id(&out);
    let tile = |row: u32, col: u32| json!({ "job_id": id, "mode": "tile_image", "page": 0, "row": row, "col": col });
    let first = h.call("get_result", tile(0, 0)).await.ok()["image"].clone();
    let right = h.call("get_result", tile(0, 1)).await.ok()["image"].clone();
    let below = h.call("get_result", tile(1, 0)).await.ok()["image"].clone();
    assert_eq!(right["x"], first["width"]);
    assert_eq!(right["y"], 0);
    assert_eq!(below["y"], first["height"]);
    assert_ne!(first["path"], right["path"]);
    h.stop().await;
}

/* REQ-MCP-113: every label of a page is inside at least one of its tiles */
#[tokio::test]
async fn tiles_together_cover_every_label() {
    let h = Harness::start().await;
    let out = h.parse_file(&h.plan()).await;
    let id = job_id(&out);
    let page = &out["summary"]["pages"][0];
    let mut in_tiles = BTreeSet::new();
    for row in 0..page["tile_rows"].as_u64().unwrap() {
        for col in 0..page["tile_cols"].as_u64().unwrap() {
            let args = json!({ "job_id": id, "mode": "tile_image", "page": 0, "row": row, "col": col });
            in_tiles.extend(label_texts(&h.call("get_result", args).await.ok()));
        }
    }
    let all = h.call("get_result", json!({ "job_id": id, "mode": "labels", "page": 0 })).await.ok();
    assert_eq!(in_tiles, label_texts(&all).into_iter().collect::<BTreeSet<_>>());
    h.stop().await;
}

/* REQ-MCP-114: tile_image requires row and col inside the tile grid */
#[tokio::test]
async fn tile_coordinates_are_checked() {
    let h = Harness::start().await;
    let out = h.parse_file(&h.plan()).await;
    let id = job_id(&out);
    let missing = h.call("get_result", json!({ "job_id": id, "mode": "tile_image", "page": 0, "row": 0 })).await;
    assert_eq!(missing.err_code(), "invalid_argument");
    assert!(missing.text.contains("rows") && missing.text.contains("columns"));
    let outside = h
        .call("get_result", json!({ "job_id": id, "mode": "tile_image", "page": 0, "row": 9, "col": 9 }))
        .await;
    assert_eq!(outside.err_code(), "invalid_argument");
    h.stop().await;
}

/* REQ-MCP-115: tile_image on a drawing parsed without tiles says how to get tiles */
#[tokio::test]
async fn tile_image_needs_a_parse_with_tiles() {
    let h = Harness::start().await;
    let out = h.call("parse_file", json!({ "path": h.plan(), "size": 1024, "tiles": false })).await.ok();
    let args = json!({ "job_id": job_id(&out), "mode": "tile_image", "page": 0, "row": 0, "col": 0 });
    let reply = h.call("get_result", args).await;
    assert_eq!(reply.err_code(), "invalid_argument");
    assert!(reply.text.contains("tiles=true"));
    h.stop().await;
}

/* REQ-MCP-119: a page no larger than one tile has no tiles and tile_image points to page_image */
#[tokio::test]
async fn small_page_has_no_tiles() {
    let h = Harness::start().await;
    let out = h.call("parse_file", json!({ "path": h.plan(), "size": 1024, "tiles": true })).await.ok();
    let page = &out["summary"]["pages"][0];
    assert_eq!((page["tile_rows"].clone(), page["tile_cols"].clone()), (json!(0), json!(0)));
    let args = json!({ "job_id": job_id(&out), "mode": "tile_image", "page": 0, "row": 0, "col": 0 });
    let reply = h.call("get_result", args).await;
    assert_eq!(reply.err_code(), "invalid_argument");
    assert!(reply.text.contains("page_image") && reply.text.contains("no tiles"), "{}", reply.text);
    h.stop().await;
}

/* REQ-MCP-116: reading a result creates no job and costs nothing */
#[tokio::test]
async fn reading_costs_nothing() {
    let h = Harness::start().await;
    let (id, _) = parsed_plan(&h).await;
    for mode in ["summary", "index", "labels"] {
        h.call("get_result", json!({ "job_id": id, "mode": mode })).await.ok();
    }
    h.call("get_result", json!({ "job_id": id, "mode": "page_image", "page": 0 })).await.ok();
    let listing = h.call("list_jobs", json!({})).await.ok();
    assert_eq!(listing["total"], 1);
    assert_eq!(listing["total_cost_units"], 1.0);
    h.stop().await;
}

/* REQ-MCP-117: a cache-hit job reads the same result as the job that parsed it */
#[tokio::test]
async fn cached_job_reads_the_same_result() {
    let h = Harness::start().await;
    let path = h.plan();
    let first = job_id(&h.parse_small(&path).await);
    let second = job_id(&h.parse_small(&path).await);
    let read = |id: String| json!({ "job_id": id, "mode": "labels" });
    let a = h.call("get_result", read(first)).await.ok();
    let b = h.call("get_result", read(second)).await.ok();
    assert_eq!(a["labels"], b["labels"]);
    h.stop().await;
}

/* REQ-MCP-118: a drawing without text has an empty label listing and still has images */
#[tokio::test]
async fn drawing_without_text_reads_cleanly() {
    let h = Harness::start().await;
    let id = job_id(&h.parse_small(&h.blank()).await);
    let labels = h.call("get_result", json!({ "job_id": id, "mode": "labels" })).await.ok();
    assert_eq!((labels["total"].clone(), labels["labels"].clone()), (json!(0), json!([])));
    let image = h.call("get_result", json!({ "job_id": id, "mode": "page_image", "page": 0 })).await;
    assert_eq!(image.images.len(), 1);
    h.stop().await;
}
