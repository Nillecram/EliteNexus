//! Minimal HTTP/1.1 request reading, responses and access checks for the MCP endpoint.
//!
//! One request per connection (every response says `Connection: close`). Nothing here trusts the peer: header
//! lines and counts are capped, the body is capped before it is read, and a request that is not plainly
//! well-formed is a `BadRequest`.

use std::net::IpAddr;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

/// Largest request body accepted (1 MiB).
pub const MAX_BODY: usize = 1 << 20;
/// Longest header line (and request line) accepted.
const MAX_LINE: usize = 8 * 1024;
/// Most header lines accepted.
const MAX_HEADERS: usize = 64;

/// A parsed request. Header names are lower-cased.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// The first header called `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Why a request could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpError {
    /// 400: not a well-formed request.
    BadRequest,
    /// 411: a body without `Content-Length` (chunked transfer is not supported).
    LengthRequired,
    /// 413: `Content-Length` over the cap.
    TooLarge,
    /// The peer closed before sending a request line: there is nobody to answer.
    Closed,
}

impl HttpError {
    /// The status to answer with (`Closed` gets no reply; 400 is only there so a blind caller stays sane).
    pub fn status(&self) -> u16 {
        match self {
            HttpError::BadRequest | HttpError::Closed => 400,
            HttpError::LengthRequired => 411,
            HttpError::TooLarge => 413,
        }
    }
}

/// Reads one line (without its line ending), at most `MAX_LINE` bytes. `Ok(None)` is a clean end of stream before
/// any byte; a line that is too long, truncated or not UTF-8 is a `BadRequest`.
async fn read_line<R: AsyncBufRead + Unpin>(r: &mut R) -> Result<Option<String>, HttpError> {
    let mut buf = Vec::new();
    let n = r.take(MAX_LINE as u64 + 1).read_until(b'\n', &mut buf).await.map_err(|_| HttpError::BadRequest)?;
    if n == 0 {
        return Ok(None);
    }
    if buf.last() != Some(&b'\n') {
        return Err(HttpError::BadRequest); // too long, or the stream ended mid-line
    }
    buf.pop();
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    String::from_utf8(buf).map(Some).map_err(|_| HttpError::BadRequest)
}

/// Reads one request: request line, headers, then a body of exactly `Content-Length` bytes (at most `max_body`).
/// A body-carrying method (POST, PUT, PATCH) without `Content-Length`, or any `Transfer-Encoding`, is a 411.
pub async fn read_request<R: AsyncBufRead + Unpin>(r: &mut R, max_body: usize) -> Result<Request, HttpError> {
    let line = read_line(r).await?.ok_or(HttpError::Closed)?;
    let mut parts = line.split(' ');
    let (method, path, version) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(m), Some(p), Some(v), None) => (m, p, v),
        _ => return Err(HttpError::BadRequest),
    };
    if method.is_empty() || !method.bytes().all(|b| b.is_ascii_uppercase()) || !path.starts_with('/') || !version.starts_with("HTTP/") {
        return Err(HttpError::BadRequest);
    }

    let mut headers: Vec<(String, String)> = Vec::new();
    loop {
        let line = read_line(r).await?.ok_or(HttpError::BadRequest)?;
        if line.is_empty() {
            break;
        }
        if headers.len() >= MAX_HEADERS {
            return Err(HttpError::BadRequest);
        }
        let (name, value) = line.split_once(':').ok_or(HttpError::BadRequest)?;
        if name.is_empty() || name.bytes().any(|b| b.is_ascii_whitespace() || b.is_ascii_control()) {
            return Err(HttpError::BadRequest);
        }
        headers.push((name.to_ascii_lowercase(), value.trim().to_string()));
    }

    let mut req = Request { method: method.to_string(), path: path.to_string(), headers, body: Vec::new() };
    if req.header("transfer-encoding").is_some() {
        return Err(HttpError::LengthRequired);
    }
    let mut length: Option<usize> = None;
    for (_, v) in req.headers.iter().filter(|(k, _)| k == "content-length") {
        if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
            return Err(HttpError::BadRequest);
        }
        // More digits than any usize needs is simply too large (and never overflows).
        let n = v.parse::<usize>().unwrap_or(usize::MAX);
        if length.is_some_and(|l| l != n) {
            return Err(HttpError::BadRequest); // two different lengths
        }
        length = Some(n);
    }
    let len = match length {
        Some(n) => n,
        None if matches!(req.method.as_str(), "POST" | "PUT" | "PATCH") => return Err(HttpError::LengthRequired),
        None => 0,
    };
    if len > max_body {
        return Err(HttpError::TooLarge);
    }
    if len > 0 {
        let mut body = vec![0u8; len];
        r.read_exact(&mut body).await.map_err(|_| HttpError::BadRequest)?;
        req.body = body;
    }
    Ok(req)
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "Status",
    }
}

/// A complete HTTP/1.1 response, always with `Content-Length` and `Connection: close`.
pub fn response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut head = format!("HTTP/1.1 {status} {}\r\n", reason(status));
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()));
    let mut out = head.into_bytes();
    out.extend_from_slice(body);
    out
}

/// Equality without an early exit on the first differing byte (the length still shows, which is fine for a key of
/// fixed length).
pub(crate) fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

/// The host of an `Origin` value (`scheme://host[:port]`), lower-cased, brackets kept for IPv6; `None` if it has no
/// scheme (`null` included).
fn origin_host(origin: &str) -> Option<String> {
    let (_, rest) = origin.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(""); // `http://localhost@evil.example` is evil.example
    let host = if authority.starts_with('[') { authority.split_once(']').map(|(h, _)| format!("{h}]"))? } else { authority.split(':').next().unwrap_or("").to_string() };
    Some(host.to_ascii_lowercase())
}

/// Access rules for the endpoint. A request whose `Origin` names any host other than `localhost`, `127.0.0.1` or
/// `[::1]` is a 403 (a web page in the commander's browser must not reach this server). A peer that is not on the
/// loopback interface must also send `Authorization: Bearer <key>` (401 otherwise, and always if `key` is empty).
pub fn check_access(req: &Request, peer: IpAddr, key: &str) -> Result<(), u16> {
    for (_, origin) in req.headers.iter().filter(|(k, _)| k == "origin") {
        match origin_host(origin).as_deref() {
            Some("localhost" | "127.0.0.1" | "[::1]") => {}
            _ => return Err(403),
        }
    }
    if peer.to_canonical().is_loopback() {
        return Ok(());
    }
    let token = req.header("authorization").and_then(|v| v.split_once(' ')).filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer")).map(|(_, t)| t.trim());
    match token {
        Some(t) if !key.is_empty() && ct_eq(t.as_bytes(), key.as_bytes()) => Ok(()),
        _ => Err(401),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    async fn read(bytes: &[u8]) -> Result<Request, HttpError> {
        read_request(&mut tokio::io::BufReader::new(bytes), MAX_BODY).await
    }

    fn req(headers: &[(&str, &str)]) -> Request {
        Request { method: "POST".into(), path: "/mcp".into(), headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), body: Vec::new() }
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[tokio::test]
    async fn reads_a_post_with_body() {
        let r = read(b"POST /mcp HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\nAuthorization: Bearer k\r\n\r\n{}").await.unwrap();
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, "/mcp");
        assert_eq!(r.header("authorization"), Some("Bearer k"));
        assert_eq!(r.header("Authorization"), Some("Bearer k"));
        assert_eq!(r.body, b"{}");
    }

    #[tokio::test]
    async fn chunked_or_missing_length_is_411() {
        assert_eq!(read(b"POST /mcp HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n").await.unwrap_err(), HttpError::LengthRequired);
        assert_eq!(read(b"POST /mcp HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap_err(), HttpError::LengthRequired);
        assert_eq!(HttpError::LengthRequired.status(), 411);
    }

    #[tokio::test]
    async fn oversized_body_is_413() {
        // The body is never sent: the error comes from the header alone.
        assert_eq!(read(b"POST /mcp HTTP/1.1\r\nContent-Length: 1048577\r\n\r\n").await.unwrap_err(), HttpError::TooLarge);
        assert_eq!(HttpError::TooLarge.status(), 413);
    }

    #[tokio::test]
    async fn garbage_is_400() {
        assert_eq!(read(b"\x00\x01 nonsense\r\n\r\n").await.unwrap_err(), HttpError::BadRequest);
        assert_eq!(read(b"GET /mcp HTTP/1.1\r\nno colon here\r\n\r\n").await.unwrap_err(), HttpError::BadRequest);
        assert_eq!(HttpError::BadRequest.status(), 400);
    }

    #[tokio::test]
    async fn get_needs_no_length() {
        let r = read(b"GET /mcp HTTP/1.1\r\n\r\n").await.unwrap();
        assert_eq!(r.method, "GET");
        assert!(r.body.is_empty());
    }

    #[tokio::test]
    async fn header_caps_are_400() {
        let mut many = b"GET /mcp HTTP/1.1\r\n".to_vec();
        for i in 0..65 {
            many.extend_from_slice(format!("X-{i}: v\r\n").as_bytes());
        }
        many.extend_from_slice(b"\r\n");
        assert_eq!(read(&many).await.unwrap_err(), HttpError::BadRequest);
        let long = format!("GET /mcp HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(9000));
        assert_eq!(read(long.as_bytes()).await.unwrap_err(), HttpError::BadRequest);
        // A body shorter than its Content-Length is a bad request, not a hang.
        assert_eq!(read(b"POST /mcp HTTP/1.1\r\nContent-Length: 5\r\n\r\n{}").await.unwrap_err(), HttpError::BadRequest);
    }

    #[tokio::test]
    async fn closed_before_a_request_line() {
        assert_eq!(read(b"").await.unwrap_err(), HttpError::Closed);
    }

    #[test]
    fn loopback_needs_no_key() {
        assert_eq!(check_access(&req(&[]), ip("127.0.0.1"), "k"), Ok(()));
        assert_eq!(check_access(&req(&[]), ip("::1"), "k"), Ok(()));
    }

    #[test]
    fn lan_peer_needs_the_key() {
        let lan = ip("192.168.1.5");
        assert_eq!(check_access(&req(&[]), lan, "k"), Err(401));
        assert_eq!(check_access(&req(&[("authorization", "Bearer wrong")]), lan, "k"), Err(401));
        assert_eq!(check_access(&req(&[("authorization", "Bearer k")]), lan, "k"), Ok(()));
        assert_eq!(check_access(&req(&[("authorization", "Bearer ")]), lan, ""), Err(401));
        assert_eq!(check_access(&req(&[("authorization", "Bearer k")]), lan, ""), Err(401));
        assert_eq!(check_access(&req(&[("authorization", "k")]), lan, "k"), Err(401));
    }

    #[test]
    fn foreign_origin_is_403() {
        let lo = ip("127.0.0.1");
        assert_eq!(check_access(&req(&[("origin", "http://evil.example")]), lo, "k"), Err(403));
        assert_eq!(check_access(&req(&[("origin", "null")]), lo, "k"), Err(403));
        assert_eq!(check_access(&req(&[("origin", "http://localhost.evil.example")]), lo, "k"), Err(403));
        assert_eq!(check_access(&req(&[("origin", "http://localhost@evil.example")]), lo, "k"), Err(403));
        for ok in ["http://localhost:3000", "http://127.0.0.1", "http://[::1]:8080", "https://localhost"] {
            assert_eq!(check_access(&req(&[("origin", ok)]), lo, "k"), Ok(()), "{ok}");
        }
        // Origin is checked before the key.
        assert_eq!(check_access(&req(&[("origin", "http://evil.example")]), ip("192.168.1.5"), "k"), Err(403));
    }

    #[test]
    fn response_has_length_and_close() {
        let r = String::from_utf8(response(202, &[], b"")).unwrap();
        assert!(r.starts_with("HTTP/1.1 202 Accepted\r\n"), "{r}");
        assert!(r.contains("Content-Length: 0\r\n"));
        assert!(r.contains("Connection: close\r\n"));
        let r = String::from_utf8(response(200, &[("Content-Type", "application/json")], b"{}")).unwrap();
        assert!(r.contains("Content-Type: application/json\r\n"));
        assert!(r.contains("Content-Length: 2\r\n"));
        assert!(r.ends_with("\r\n\r\n{}"));
    }

    #[test]
    fn constant_time_compare() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
    }
}
