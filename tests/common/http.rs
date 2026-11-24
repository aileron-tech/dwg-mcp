//! A tiny HTTP/1.1 server on localhost for the parse_url tests. It serves
//! fixed routes and counts the requests each one received.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Route {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Content-Length to announce instead of the real body length.
    pub declared_len: Option<u64>,
}

impl Route {
    pub fn ok(content_type: &str, body: Vec<u8>) -> Route {
        Route {
            status: 200,
            headers: vec![("Content-Type".into(), content_type.into())],
            body,
            declared_len: None,
        }
    }

    pub fn dwg(body: Vec<u8>) -> Route {
        Route::ok("application/acad", body)
    }

    pub fn status(status: u16) -> Route {
        Route { status, headers: Vec::new(), body: b"error".to_vec(), declared_len: None }
    }

    pub fn redirect(to: &str) -> Route {
        Route {
            status: 302,
            headers: vec![("Location".into(), to.into())],
            body: Vec::new(),
            declared_len: None,
        }
    }

    pub fn declared_len(mut self, len: u64) -> Route {
        self.declared_len = Some(len);
        self
    }

    pub fn header(mut self, name: &str, value: &str) -> Route {
        self.headers.push((name.into(), value.into()));
        self
    }
}

pub struct FileServer {
    pub base: String,
    routes: Arc<Mutex<HashMap<String, Route>>>,
    hits: Arc<Mutex<HashMap<String, u32>>>,
}

impl FileServer {
    pub fn start() -> FileServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let routes: Arc<Mutex<HashMap<String, Route>>> = Arc::default();
        let hits: Arc<Mutex<HashMap<String, u32>>> = Arc::default();
        let (r, h) = (Arc::clone(&routes), Arc::clone(&hits));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream);
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(n) if n > 2 => continue,
                        _ => break,
                    }
                }
                let target = request_line.split_whitespace().nth(1).unwrap_or("/").to_string();
                let path = target.split('?').next().unwrap_or("/").to_string();
                *h.lock().unwrap().entry(path.clone()).or_insert(0) += 1;
                let route = r.lock().unwrap().get(&path).cloned().unwrap_or(Route::status(404));
                let mut head = format!(
                    "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                    route.status,
                    route.declared_len.unwrap_or(route.body.len() as u64)
                );
                for (name, value) in &route.headers {
                    head.push_str(&format!("{name}: {value}\r\n"));
                }
                head.push_str("\r\n");
                let mut stream = reader.into_inner();
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&route.body);
                let _ = stream.flush();
            }
        });
        FileServer { base, routes, hits }
    }

    pub fn route(&self, path: &str, route: Route) -> String {
        self.routes.lock().unwrap().insert(path.to_string(), route);
        format!("{}{path}", self.base)
    }

    /// Requests received for `path` so far.
    pub fn hits(&self, path: &str) -> u32 {
        self.hits.lock().unwrap().get(path).copied().unwrap_or(0)
    }
}
