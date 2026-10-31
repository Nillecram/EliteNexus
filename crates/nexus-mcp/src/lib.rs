//! MCP (Model Context Protocol) server for Elite Nexus: lets an outside AI client (Claude Desktop, Claude Code, ...)
//! call the same read-only tools the built-in assistant has. This crate only serves tools: it reads HTTP requests,
//! checks who is asking and answers JSON-RPC; the app implements the tools and starts the server.

pub mod http;
