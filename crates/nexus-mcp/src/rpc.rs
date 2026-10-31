//! JSON-RPC dispatch of MCP methods over a `ToolHost`. Pure: no sockets, no state (no `Mcp-Session-Id` is minted).
//!
//! Two protocol eras are served on the same endpoint, decided per request. The current revision (2026-07-28,
//! "modern") is stateless: no `initialize`, every request carries its protocol version and client capabilities in
//! `params._meta`, and the `MCP-Protocol-Version`, `Mcp-Method` and (for `tools/call`) `Mcp-Name` headers repeat them.
//! The 2025 revisions ("legacy") open with `initialize`.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use nexus_ai::ToolHost;
use serde_json::{Value, json};

/// Protocol revisions served the stateless way.
pub const MODERN_VERSIONS: &[&str] = &["2026-07-28"];
/// Protocol revisions that open with `initialize`, newest first.
pub const LEGACY_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// What the server says about itself.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub version: String,
    pub instructions: String,
}

/// The `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` header values of the request, when present.
#[derive(Debug, Clone, Copy, Default)]
pub struct RpcHeaders<'a> {
    pub protocol_version: Option<&'a str>,
    pub method: Option<&'a str>,
    pub name: Option<&'a str>,
}

/// An HTTP status and JSON body; `body: None` means 202 with no body.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: Option<Value>,
}

/// An error to put in a JSON-RPC reply: code, message, optional data.
type RpcError = (i64, String, Option<Value>);

fn supported_versions() -> Vec<&'static str> {
    MODERN_VERSIONS.iter().chain(LEGACY_VERSIONS).copied().collect()
}

fn server_info(info: &ServerInfo) -> Value {
    json!({"name": "elite-nexus", "title": "Elite Nexus", "version": info.version})
}

fn ok(id: &Value, result: Value) -> Reply {
    Reply { status: 200, body: Some(json!({"jsonrpc": "2.0", "id": id, "result": result})) }
}

fn fail(status: u16, id: &Value, (code, message, data): RpcError) -> Reply {
    let mut error = json!({"code": code, "message": message});
    if let Some(data) = data {
        error["data"] = data;
    }
    Reply { status, body: Some(json!({"jsonrpc": "2.0", "id": id, "error": error})) }
}

fn err(code: i64, message: &str) -> RpcError {
    (code, message.to_string(), None)
}

/// Answer one JSON-RPC message. The caller has already checked who is asking and parsed the body as JSON.
pub fn handle(msg: &Value, h: &RpcHeaders, host: &dyn ToolHost, info: &ServerInfo) -> Reply {
    let Some(obj) = msg.as_object() else {
        return fail(400, &Value::Null, err(-32600, "Invalid request: expected a single JSON object (batches are not supported)"));
    };
    // A client response (no method) or a notification (no id) gets no answer in either era.
    let (Some(method), Some(id)) = (obj.get("method"), obj.get("id")) else {
        return Reply { status: 202, body: None };
    };
    let Some(method) = method.as_str() else {
        return fail(400, id, err(-32600, "Invalid request: method must be a string"));
    };
    let params = obj.get("params").unwrap_or(&Value::Null);
    let meta_version = params.get("_meta").and_then(|m| m.get(META_VERSION));
    if meta_version.is_some() || h.protocol_version.is_some_and(|v| MODERN_VERSIONS.contains(&v)) {
        modern(id, method, params, meta_version, h, host, info)
    } else {
        legacy(id, method, params, h, host, info)
    }
}

/// 2026-07-28: stateless, everything needed is on the request.
fn modern(id: &Value, method: &str, params: &Value, meta_version: Option<&Value>, h: &RpcHeaders, host: &dyn ToolHost, info: &ServerInfo) -> Reply {
    let Some(requested) = meta_version else {
        return fail(400, id, err(-32602, "Invalid params: _meta[\"io.modelcontextprotocol/protocolVersion\"] is required"));
    };
    let version = requested.as_str().filter(|v| supported_versions().contains(v));
    let Some(version) = version else {
        let data = json!({"supported": supported_versions(), "requested": requested});
        return fail(400, id, (-32022, "Unsupported protocol version".to_string(), Some(data)));
    };
    if params.get("_meta").and_then(|m| m.get(META_CAPABILITIES)).is_none() {
        return fail(400, id, err(-32602, "Invalid params: _meta[\"io.modelcontextprotocol/clientCapabilities\"] is required"));
    }
    if let Some(problem) = header_mismatch(method, params, version, h) {
        return fail(400, id, (-32020, format!("Header mismatch: {problem}"), None));
    }
    let result = match method {
        "server/discover" => Ok(json!({
            "supportedVersions": supported_versions(),
            "capabilities": {"tools": {}},
            "instructions": info.instructions,
        })),
        "tools/list" => Ok(tools_list(host)),
        "tools/call" => tools_call(host, params),
        _ => return fail(404, id, err(-32601, "Method not found")),
    };
    match result {
        Ok(mut result) => {
            result["resultType"] = json!("complete");
            result["_meta"] = json!({META_SERVER_INFO: server_info(info)});
            ok(id, result)
        }
        Err(e) => fail(200, id, e),
    }
}

/// What differs between the headers and the body, if anything.
fn header_mismatch(method: &str, params: &Value, version: &str, h: &RpcHeaders) -> Option<String> {
    if h.protocol_version != Some(version) {
        return Some(format!("MCP-Protocol-Version does not match _meta protocolVersion {version}"));
    }
    if h.method != Some(method) {
        return Some(format!("Mcp-Method does not match the request method {method}"));
    }
    if method == "tools/call" {
        let wanted = params.get("name").and_then(Value::as_str);
        if h.name.map(decode_name).as_deref() != wanted || wanted.is_none() {
            return Some("Mcp-Name does not match params.name".to_string());
        }
    }
    None
}

/// An `Mcp-Name` of the form `=?base64?<b64>?=` stands for the UTF-8 text it encodes; anything else is literal.
fn decode_name(raw: &str) -> String {
    let encoded = raw.strip_prefix("=?base64?").and_then(|r| r.strip_suffix("?="));
    match encoded {
        Some(b64) => STANDARD.decode(b64).ok().and_then(|b| String::from_utf8(b).ok()).unwrap_or_else(|| raw.to_string()),
        None => raw.to_string(),
    }
}

/// 2025 revisions: `initialize` first, errors in the body with HTTP 200.
fn legacy(id: &Value, method: &str, params: &Value, h: &RpcHeaders, host: &dyn ToolHost, info: &ServerInfo) -> Reply {
    if h.protocol_version.is_some_and(|v| !LEGACY_VERSIONS.contains(&v)) {
        return fail(400, id, err(-32600, "Unsupported MCP-Protocol-Version header"));
    }
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked.filter(|v| LEGACY_VERSIONS.contains(v)).unwrap_or(LEGACY_VERSIONS[0]);
            Ok(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {}},
                "serverInfo": server_info(info),
                "instructions": info.instructions,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools_list(host)),
        "tools/call" => tools_call(host, params),
        _ => Err(err(-32601, "Method not found")),
    };
    match result {
        Ok(result) => ok(id, result),
        Err(e) => fail(200, id, e),
    }
}

fn tools_list(host: &dyn ToolHost) -> Value {
    let tools: Vec<Value> = host.tools().into_iter().map(|s| json!({"name": s.name, "description": s.description, "inputSchema": s.parameters})).collect();
    json!({"tools": tools})
}

fn tools_call(host: &dyn ToolHost, params: &Value) -> Result<Value, RpcError> {
    let name = params.get("name").and_then(Value::as_str).ok_or_else(|| err(-32602, "Invalid params: name is required"))?;
    if !host.tools().iter().any(|s| s.name == name) {
        return Err((-32602, format!("Unknown tool: {name}"), None));
    }
    let args = match params.get("arguments") {
        None => json!({}),
        Some(a) if a.is_object() => a.clone(),
        Some(_) => return Err(err(-32602, "Invalid params: arguments must be an object")),
    };
    let (text, is_error) = match host.call(name, &args) {
        Ok(v) => (serde_json::to_string(&v).unwrap_or_else(|_| "null".to_string()), false),
        Err(s) => (s, true),
    };
    Ok(json!({"content": [{"type": "text", "text": text}], "isError": is_error}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_ai::ToolSpec;

    struct Fake;
    impl ToolHost for Fake {
        fn tools(&self) -> Vec<ToolSpec> {
            vec![
                ToolSpec { name: "echo", description: "Returns its arguments.", parameters: json!({"type": "object", "properties": {"a": {"type": "integer"}}}) },
                ToolSpec { name: "fail", description: "Always fails.", parameters: json!({"type": "object"}) },
            ]
        }
        fn call(&self, name: &str, args: &Value) -> Result<Value, String> {
            match name {
                "echo" => Ok(args.clone()),
                "fail" => Err("nope".into()),
                _ => Err("unknown".into()),
            }
        }
    }

    fn info() -> ServerInfo {
        ServerInfo { version: "9.9.9".into(), instructions: "Use the tools.".into() }
    }

    fn run(msg: Value, h: RpcHeaders) -> Reply {
        handle(&msg, &h, &Fake, &info())
    }

    fn legacy_run(msg: Value) -> Reply {
        run(msg, RpcHeaders::default())
    }

    /// A modern request with `_meta` and the matching headers.
    fn modern_run(method: &str, params: Value, name: Option<&str>) -> Reply {
        run(modern_msg(method, params), RpcHeaders { protocol_version: Some("2026-07-28"), method: Some(method), name })
    }

    fn modern_msg(method: &str, mut params: Value) -> Value {
        params["_meta"] = json!({META_VERSION: "2026-07-28", META_CAPABILITIES: {}});
        json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
    }

    fn body(r: &Reply) -> &Value {
        r.body.as_ref().unwrap_or(&Value::Null)
    }

    fn code(r: &Reply) -> i64 {
        body(r)["error"]["code"].as_i64().unwrap_or(0)
    }

    #[test]
    fn legacy_initialize_answers_the_clients_version() {
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}));
        assert_eq!(r.status, 200);
        let res = &body(&r)["result"];
        assert_eq!(res["protocolVersion"], "2025-06-18");
        assert!(res["capabilities"]["tools"].is_object());
        assert_eq!(res["serverInfo"]["name"], "elite-nexus");
        assert_eq!(res["serverInfo"]["title"], "Elite Nexus");
        assert_eq!(res["serverInfo"]["version"], "9.9.9");
        assert_eq!(res["instructions"], info().instructions);
    }

    #[test]
    fn legacy_initialize_offers_newest_for_unknown() {
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "1999-01-01"}}));
        assert_eq!(body(&r)["result"]["protocolVersion"], "2025-11-25");
    }

    #[test]
    fn notifications_and_client_responses_get_202() {
        let r = legacy_run(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        assert_eq!((r.status, r.body), (202, None));
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 1, "result": {}}));
        assert_eq!((r.status, r.body), (202, None));
    }

    #[test]
    fn ping_is_empty_result() {
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 7, "method": "ping"}));
        assert_eq!(r.status, 200);
        assert_eq!(body(&r), &json!({"jsonrpc": "2.0", "id": 7, "result": {}}));
    }

    #[test]
    fn tools_list_maps_specs() {
        let want = json!({"name": "echo", "description": "Returns its arguments.", "inputSchema": {"type": "object", "properties": {"a": {"type": "integer"}}}});
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
        assert_eq!(r.status, 200);
        assert_eq!(body(&r)["result"]["tools"][0], want);
        assert_eq!(body(&r)["result"]["tools"][1]["name"], "fail");
        assert!(body(&r)["result"].get("resultType").is_none());
        let r = modern_run("tools/list", json!({}), None);
        assert_eq!(r.status, 200);
        assert_eq!(body(&r)["result"]["tools"][0], want);
        assert_eq!(body(&r)["result"]["resultType"], "complete");
    }

    #[test]
    fn call_wraps_result_as_text() {
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "echo", "arguments": {"a": 1}}}));
        assert_eq!(r.status, 200);
        let res = &body(&r)["result"];
        assert_eq!(res["content"][0], json!({"type": "text", "text": "{\"a\":1}"}));
        assert_eq!(res["isError"], false);
    }

    #[test]
    fn tool_error_is_is_error_not_protocol_error() {
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "fail"}}));
        assert_eq!(r.status, 200);
        assert!(body(&r).get("error").is_none());
        let res = &body(&r)["result"];
        assert_eq!(res["isError"], true);
        assert_eq!(res["content"][0]["text"], "nope");
    }

    #[test]
    fn unknown_tool_and_bad_args_are_32602() {
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "nope"}}));
        assert_eq!((r.status, code(&r)), (200, -32602));
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "echo", "arguments": "x"}}));
        assert_eq!((r.status, code(&r)), (200, -32602));
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "echo"}}));
        assert_eq!(body(&r)["result"]["content"][0]["text"], "{}");
    }

    #[test]
    fn unknown_method_legacy_200_modern_404() {
        let r = legacy_run(json!({"jsonrpc": "2.0", "id": "abc", "method": "resources/list"}));
        assert_eq!((r.status, code(&r)), (200, -32601));
        assert_eq!(body(&r)["id"], "abc");
        let r = modern_run("resources/list", json!({}), None);
        assert_eq!((r.status, code(&r)), (404, -32601));
        assert_eq!(body(&r)["id"], 1);
    }

    #[test]
    fn batches_and_non_objects_are_32600() {
        for msg in [json!([]), json!([{"jsonrpc": "2.0", "id": 1, "method": "ping"}]), json!(5)] {
            let r = legacy_run(msg);
            assert_eq!((r.status, code(&r)), (400, -32600));
            assert!(body(&r)["id"].is_null());
        }
    }

    #[test]
    fn modern_discover() {
        let r = modern_run("server/discover", json!({}), None);
        assert_eq!(r.status, 200);
        let res = &body(&r)["result"];
        assert_eq!(res["supportedVersions"], json!(["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26"]));
        assert_eq!(res["resultType"], "complete");
        assert_eq!(res["_meta"][META_SERVER_INFO]["name"], "elite-nexus");
        assert_eq!(res["instructions"], info().instructions);
        assert!(res["capabilities"]["tools"].is_object());
    }

    #[test]
    fn modern_unsupported_version_is_32022() {
        let mut msg = modern_msg("tools/list", json!({}));
        msg["params"]["_meta"][META_VERSION] = json!("1900-01-01");
        let r = run(msg, RpcHeaders { protocol_version: Some("1900-01-01"), method: Some("tools/list"), name: None });
        assert_eq!((r.status, code(&r)), (400, -32022));
        let data = &body(&r)["error"]["data"];
        assert_eq!(data["requested"], "1900-01-01");
        assert_eq!(data["supported"], json!(["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26"]));
    }

    #[test]
    fn modern_missing_capabilities_is_32602() {
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {"_meta": {META_VERSION: "2026-07-28"}}});
        let r = run(msg, RpcHeaders { protocol_version: Some("2026-07-28"), method: Some("tools/list"), name: None });
        assert_eq!((r.status, code(&r)), (400, -32602));
    }

    #[test]
    fn modern_header_mismatch_is_32020() {
        let msg = modern_msg("tools/call", json!({"name": "echo"}));
        let good = RpcHeaders { protocol_version: Some("2026-07-28"), method: Some("tools/call"), name: Some("echo") };
        assert_eq!(run(msg.clone(), good).status, 200);
        for h in [
            RpcHeaders { method: Some("tools/list"), ..good },
            RpcHeaders { method: None, ..good },
            RpcHeaders { name: Some("other"), ..good },
            RpcHeaders { name: None, ..good },
            RpcHeaders { protocol_version: Some("2025-06-18"), ..good },
            RpcHeaders { protocol_version: None, ..good },
        ] {
            let r = run(msg.clone(), h);
            assert_eq!((r.status, code(&r)), (400, -32020), "{h:?}");
        }
    }

    #[test]
    fn modern_base64_name_is_decoded() {
        let r = modern_run("tools/call", json!({"name": "echo"}), Some("=?base64?ZWNobw==?="));
        assert_eq!(r.status, 200);
        assert_eq!(body(&r)["result"]["isError"], false);
    }

    #[test]
    fn modern_header_without_meta_is_32602() {
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
        let r = run(msg, RpcHeaders { protocol_version: Some("2026-07-28"), method: Some("tools/list"), name: None });
        assert_eq!((r.status, code(&r)), (400, -32602));
    }

    #[test]
    fn legacy_bad_version_header_is_400() {
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
        let r = run(msg.clone(), RpcHeaders { protocol_version: Some("1999-01-01"), ..Default::default() });
        assert_eq!((r.status, code(&r)), (400, -32600));
        let r = run(msg, RpcHeaders { protocol_version: Some("2025-06-18"), ..Default::default() });
        assert_eq!(r.status, 200);
    }
}
