//! HTTP entrypoint checks against the real MCP router and service.

use std::net::TcpListener;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

const TOKEN: &str = "test-token-with-at-least-thirty-two-characters";

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

/* REQ-MCP-212: remote MCP requires authentication and does not expose local tools */
#[test]
fn http_authenticates_and_exposes_only_remote_tools() {
    let cache = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let _server = Server(
        Command::new(env!("CARGO_BIN_EXE_dwg-mcp"))
            .arg("--http")
            .env("DWG_MCP_HTTP_BIND", address.to_string())
            .env("DWG_MCP_HTTP_HOSTS", "127.0.0.1")
            .env("DWG_MCP_HTTP_TOKEN", TOKEN)
            .env("DWG_MCP_CACHE_DIR", cache.path())
            .spawn()
            .unwrap(),
    );
    let url = format!("http://{address}/mcp");
    let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}
    }});
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let request = |token: Option<&str>, value: &Value| {
        let mut req = agent
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2025-03-26");
        if let Some(token) = token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        req.header("Content-Type", "application/json")
            .send(value.to_string())
    };
    let unauthorized = (0..60)
        .find_map(|_| {
            let response = request(None, &initialize).ok();
            if response.is_none() {
                thread::sleep(Duration::from_millis(25));
            }
            response
        })
        .expect("HTTP server did not start");
    assert_eq!(unauthorized.status(), 401);
    assert_eq!(request(Some("wrong"), &initialize).unwrap().status(), 401);
    assert_eq!(request(Some(TOKEN), &initialize).unwrap().status(), 200);

    let list = json!({"jsonrpc":"2.0","id":2,"method":"tools/list"});
    let mut response = request(Some(TOKEN), &list).unwrap();
    let tools: Value =
        serde_json::from_str(&response.body_mut().read_to_string().unwrap()).unwrap();
    let mut names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["get_job", "get_result", "parse_url"]);

    let tool_call = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
        "name":"parse_url","arguments":{"url":"https://127.0.0.1/example.dwg"}
    }});
    let mut response = request(Some(TOKEN), &tool_call).unwrap();
    let result = response.body_mut().read_to_string().unwrap();
    assert!(result.contains("download_failed"), "{result}");
    assert!(result.contains("host not found"), "{result}");
}
