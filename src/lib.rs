//! dwg-mcp: an MCP server that turns AutoCAD DWG drawings into data an AI
//! application can use. Eight tools over stdio; the rendering is done by the
//! `dwg2png` crate.
//!
//! `contract` is the wire format, `domain` the pure rules, `store` and `fetch`
//! the filesystem and HTTP adapters, `service` the orchestration, `server` the
//! MCP entrypoint.

pub mod contract;
pub mod domain;
pub mod fetch;
pub mod server;
pub mod service;
pub mod store;
