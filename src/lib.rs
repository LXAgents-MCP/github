//! `lxagents-github`: an MCP server that gives an LLM access to GitHub through its REST API, over stdio or
//! over HTTP behind an access token.
//!
//! The layers import downward only: `routes` → `controllers` → `services` → `models` → `utils` → `config`.
//! `src/main.rs` is the binary; everything else is here so the tests can reach it.
pub mod config;
pub mod controllers;
pub mod models;
pub mod routes;
pub mod services;
pub mod utils;
