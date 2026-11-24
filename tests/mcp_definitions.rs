//! What a client sees from initialize and tools/list, and how the server
//! answers malformed calls. The tool definitions are the public contract and
//! the input of registry scoring (Glama TDQS), so they are tested as data.

mod common;

use common::{Harness, TOOL_NAMES};
use serde_json::{json, Value};

fn hint(tool: &Value, name: &str) -> bool {
    tool["annotations"][name].as_bool().unwrap_or_else(|| panic!("{} lacks {name}", tool["name"]))
}

/* REQ-MCP-001: the server identifies itself by name, title and version */
#[tokio::test]
async fn server_reports_name_title_and_version() {
    let h = Harness::start().await;
    let info = serde_json::to_value(h.client.peer_info()).unwrap();
    assert_eq!(info["serverInfo"]["name"], "dwg-mcp");
    assert_eq!(info["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(info["serverInfo"]["title"].as_str().unwrap().contains("DWG ingestion"));
    h.stop().await;
}

/* REQ-MCP-002: the server instructions name the entry tools and the cache rule */
#[tokio::test]
async fn server_instructions_name_the_entry_tools() {
    let h = Harness::start().await;
    let info = serde_json::to_value(h.client.peer_info()).unwrap();
    let text = info["instructions"].as_str().unwrap();
    for needle in ["parse_file", "parse_url", "get_result", "7 days", "one tenth"] {
        assert!(text.contains(needle), "instructions lack {needle:?}");
    }
    h.stop().await;
}

/* REQ-MCP-003: the server advertises the tools capability */
#[tokio::test]
async fn server_advertises_tools_capability() {
    let h = Harness::start().await;
    let info = serde_json::to_value(h.client.peer_info()).unwrap();
    assert!(info["capabilities"]["tools"].is_object());
    h.stop().await;
}

/* REQ-MCP-004: tools/list returns exactly the eight agreed tools */
#[tokio::test]
async fn lists_exactly_the_eight_tools() {
    let h = Harness::start().await;
    let mut names: Vec<String> =
        h.tools().await.iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    names.sort();
    let mut expected: Vec<String> = TOOL_NAMES.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(names, expected);
    h.stop().await;
}

/* REQ-MCP-005: every tool name is verb_noun in snake_case with a known verb */
#[tokio::test]
async fn tool_names_are_verb_noun_snake_case() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        let name = tool["name"].as_str().unwrap();
        assert!(name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'), "{name}");
        let verb = name.split('_').next().unwrap();
        assert!(["parse", "get", "list", "delete", "reset"].contains(&verb), "{name}");
        assert!(name.split('_').count() >= 2, "{name} has no noun");
    }
    h.stop().await;
}

/* REQ-MCP-006: every tool has a display title that says more than its name */
#[tokio::test]
async fn every_tool_has_a_meaningful_title() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        let name = tool["name"].as_str().unwrap();
        let title = tool["title"].as_str().unwrap_or_else(|| panic!("{name} has no title"));
        assert!(title != name && title.len() > name.len(), "{name}: title {title:?}");
    }
    h.stop().await;
}

/* REQ-MCP-007: every description names a sibling tool to use instead */
#[tokio::test]
async fn every_description_names_an_alternative_tool() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        let name = tool["name"].as_str().unwrap();
        let description = tool["description"].as_str().unwrap();
        let names_sibling =
            TOOL_NAMES.iter().any(|other| *other != name && description.contains(other));
        assert!(names_sibling, "{name} names no sibling tool");
        assert!(description.contains("instead"), "{name} gives no when-not-to-use guidance");
    }
    h.stop().await;
}

/* REQ-MCP-008: descriptions start with what the tool does and stay bounded */
#[tokio::test]
async fn descriptions_are_front_loaded_and_bounded() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        let name = tool["name"].as_str().unwrap();
        let description = tool["description"].as_str().unwrap();
        assert!((200..=1100).contains(&description.len()), "{name}: {} chars", description.len());
        let first_word = description.split_whitespace().next().unwrap();
        assert!(
            ["Parse", "Download", "Read", "Return", "List", "Delete", "Drop"].contains(&first_word),
            "{name} starts with {first_word:?}, not a verb"
        );
        assert_ne!(description.to_lowercase(), name);
    }
    h.stop().await;
}

/* REQ-MCP-009: every input parameter carries a description */
#[tokio::test]
async fn every_input_parameter_is_described() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        let props = tool["inputSchema"]["properties"].as_object().cloned().unwrap_or_default();
        for (param, schema) in props {
            let text = schema["description"].as_str().unwrap_or("");
            assert!(text.len() >= 20, "{}.{param} is not described", tool["name"]);
        }
    }
    h.stop().await;
}

/* REQ-MCP-010: input schemas are closed objects */
#[tokio::test]
async fn input_schemas_reject_unknown_properties() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false, "{}", tool["name"]);
    }
    h.stop().await;
}

/* REQ-MCP-011: every tool declares an output schema */
#[tokio::test]
async fn every_tool_declares_an_output_schema() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        let schema = &tool["outputSchema"];
        assert_eq!(schema["type"], "object", "{} has no output schema", tool["name"]);
        assert!(!schema["properties"].as_object().unwrap().is_empty(), "{}", tool["name"]);
    }
    h.stop().await;
}

/* REQ-MCP-012: every tool declares all four behaviour hints */
#[tokio::test]
async fn every_tool_declares_all_four_hints() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        for name in ["readOnlyHint", "destructiveHint", "idempotentHint", "openWorldHint"] {
            hint(&tool, name);
        }
    }
    h.stop().await;
}

/* REQ-MCP-013: only the three reading tools are read-only */
#[tokio::test]
async fn reading_tools_are_read_only() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        let name = tool["name"].as_str().unwrap();
        let expected = ["get_result", "get_job", "list_jobs"].contains(&name);
        assert_eq!(hint(&tool, "readOnlyHint"), expected, "{name}");
    }
    h.stop().await;
}

/* REQ-MCP-014: delete and reset tools are destructive, parse tools are not */
#[tokio::test]
async fn deleting_tools_are_marked_destructive() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        let name = tool["name"].as_str().unwrap();
        let expected = ["delete_job", "reset_file_cache", "reset_url_cache"].contains(&name);
        assert_eq!(hint(&tool, "destructiveHint"), expected, "{name}");
    }
    h.stop().await;
}

/* REQ-MCP-015: only parse_url reaches outside the machine */
#[tokio::test]
async fn only_parse_url_is_open_world() {
    let h = Harness::start().await;
    for tool in h.tools().await {
        let name = tool["name"].as_str().unwrap();
        assert_eq!(hint(&tool, "openWorldHint"), name == "parse_url", "{name}");
    }
    h.stop().await;
}

/* REQ-MCP-016: parse tools and delete_job are not idempotent; reset tools are */
#[tokio::test]
async fn idempotency_hints_match_behaviour() {
    let h = Harness::start().await;
    for name in ["parse_file", "parse_url", "delete_job"] {
        assert!(!hint(&h.tool(name).await, "idempotentHint"), "{name}");
    }
    for name in ["reset_file_cache", "reset_url_cache"] {
        assert!(hint(&h.tool(name).await, "idempotentHint"), "{name}");
    }
    h.stop().await;
}

/* REQ-MCP-017: each tool requires only its identifying argument */
#[tokio::test]
async fn required_arguments_are_minimal() {
    let h = Harness::start().await;
    let expected = [
        ("parse_file", vec!["path"]),
        ("parse_url", vec!["url"]),
        ("get_result", vec!["job_id"]),
        ("get_job", vec!["job_id"]),
        ("delete_job", vec!["job_id"]),
        ("reset_file_cache", vec!["path"]),
        ("reset_url_cache", vec!["url"]),
    ];
    for (name, required) in expected {
        assert_eq!(h.tool(name).await["inputSchema"]["required"], json!(required), "{name}");
    }
    let list_jobs = h.tool("list_jobs").await;
    assert!(list_jobs["inputSchema"]["required"].as_array().is_none_or(|r| r.is_empty()));
    h.stop().await;
}

/* REQ-MCP-018: get_result publishes its five modes in the schema */
#[tokio::test]
async fn get_result_schema_lists_the_five_modes() {
    let h = Harness::start().await;
    let text = h.tool("get_result").await["inputSchema"].to_string();
    for mode in ["summary", "index", "labels", "page_image", "tile_image"] {
        assert!(text.contains(&format!("\"{mode}\"")), "mode {mode} is not in the schema");
    }
    h.stop().await;
}

/* REQ-MCP-019: the parse tools share one output schema */
#[tokio::test]
async fn parse_tools_share_one_output_shape() {
    let h = Harness::start().await;
    let file = h.tool("parse_file").await;
    let url = h.tool("parse_url").await;
    assert_eq!(file["outputSchema"], url["outputSchema"]);
    assert_eq!(file["outputSchema"]["required"], json!(["job", "summary"]));
    h.stop().await;
}

/* REQ-MCP-020: calling a tool that does not exist is a protocol error */
#[tokio::test]
async fn unknown_tool_is_a_protocol_error() {
    let h = Harness::start().await;
    let outcome = h.call_raw("convert_drawing", json!({})).await;
    assert!(outcome.is_err() || outcome.unwrap().is_error);
    h.stop().await;
}

/* REQ-MCP-021: an argument that is not in the schema is rejected */
#[tokio::test]
async fn unknown_argument_is_rejected() {
    let h = Harness::start().await;
    let reply = h.call("get_job", json!({ "job_id": "job_x", "verbose": true })).await;
    assert!(reply.is_error);
    assert!(reply.text.contains("verbose"), "{}", reply.text);
    h.stop().await;
}

/* REQ-MCP-022: a missing required argument is rejected and named */
#[tokio::test]
async fn missing_required_argument_is_rejected() {
    let h = Harness::start().await;
    let reply = h.call("parse_file", json!({ "size": 2000 })).await;
    assert!(reply.is_error);
    assert!(reply.text.contains("path"), "{}", reply.text);
    h.stop().await;
}

/* REQ-MCP-023: an argument of the wrong type is rejected */
#[tokio::test]
async fn wrong_argument_type_is_rejected() {
    let h = Harness::start().await;
    let reply = h.call("parse_file", json!({ "path": "/tmp/a.dwg", "size": "large" })).await;
    assert!(reply.is_error);
    let reply = h.call("get_result", json!({ "job_id": "job_x", "mode": "everything" })).await;
    assert!(reply.is_error);
    h.stop().await;
}

/* REQ-MCP-024: the server survives a failed call and answers the next one */
#[tokio::test]
async fn server_keeps_serving_after_errors() {
    let h = Harness::start().await;
    assert!(h.call("get_job", json!({ "job_id": "job_missing" })).await.is_error);
    assert!(h.call("parse_file", json!({ "path": "relative.dwg" })).await.is_error);
    let listing = h.call("list_jobs", json!({})).await.ok();
    assert_eq!(listing["total"], 0);
    h.stop().await;
}
