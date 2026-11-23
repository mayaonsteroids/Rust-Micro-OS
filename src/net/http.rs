//! Hypertext Transfer Protocol (HTTP/1.1) Client.
//!
//! Provides HTTP/1.1 GET request generation, response status-line parsing,
//! header inspection (Content-Length, Transfer-Encoding: chunked), and bounded body extraction.

pub const MAX_HTTP_BODY_LEN: usize = 8192;
pub const MAX_HTTP_HEADERS_LEN: usize = 2048;

/// Parsed HTTP/1.1 Response.
pub struct HttpResponse {
    pub status_code: u16,
    pub content_length: usize,
    pub is_chunked: bool,
    pub is_truncated: bool,
    pub body_len: usize,
    pub body: [u8; MAX_HTTP_BODY_LEN],
}

impl HttpResponse {
    pub fn new() -> Self {
        Self {
            status_code: 0,
            content_length: 0,
            is_chunked: false,
            is_truncated: false,
            body_len: 0,
            body: [0u8; MAX_HTTP_BODY_LEN],
        }
    }

    /// Returns the body content as a valid UTF-8 string slice (lossy if invalid).
    pub fn body_str(&self) -> &str {
        core::str::from_utf8(&self.body[..self.body_len]).unwrap_or("<binary/non-utf8 content>")
    }
}

/// Formats an HTTP/1.1 GET request into `out_buf`.
pub fn build_http_get_request(
    hostname: &str,
    path: &str,
    out_buf: &mut [u8],
) -> Result<usize, &'static str> {
    let clean_path = if path.is_empty() { "/" } else { path };

    // Example formatted request:
    // GET / HTTP/1.1\r\n
    // Host: example.com\r\n
    // User-Agent: RustMicroOS/0.1\r\n
    // Accept: text/html, text/plain, */*\r\n
    // Connection: close\r\n
    // \r\n

    let mut idx = 0;

    let write_str = |idx: &mut usize, s: &str, buf: &mut [u8]| -> Result<(), &'static str> {
        let bytes = s.as_bytes();
        if *idx + bytes.len() > buf.len() {
            return Err("Buffer too small for HTTP request");
        }
        buf[*idx..*idx + bytes.len()].copy_from_slice(bytes);
        *idx += bytes.len();
        Ok(())
    };

    write_str(&mut idx, "GET ", out_buf)?;
    write_str(&mut idx, clean_path, out_buf)?;
    write_str(&mut idx, " HTTP/1.1\r\nHost: ", out_buf)?;
    write_str(&mut idx, hostname, out_buf)?;
    write_str(&mut idx, "\r\nUser-Agent: RustMicroOS/0.1\r\nAccept: text/html, text/plain, */*\r\nConnection: close\r\n\r\n", out_buf)?;

    Ok(idx)
}

/// Parses a raw HTTP/1.1 response stream into an `HttpResponse` structure.
pub fn parse_http_response(raw: &[u8]) -> Result<HttpResponse, &'static str> {
    if raw.is_empty() {
        return Err("Empty HTTP response stream");
    }

    // Locate header/body delimiter: "\r\n\r\n"
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("Incomplete HTTP response: missing header/body delimiter CRLF-CRLF")?;

    let header_bytes = &raw[..header_end];
    let body_bytes = &raw[header_end + 4..];

    let header_str = core::str::from_utf8(header_bytes)
        .map_err(|_| "HTTP header contains invalid UTF-8 bytes")?;

    let mut lines = header_str.split("\r\n");

    // 1. Parse Status Line: "HTTP/1.1 200 OK"
    let status_line = lines.next().ok_or("Missing HTTP status line")?;
    let mut status_parts = status_line.split_whitespace();
    let _version = status_parts.next().ok_or("Missing HTTP version")?;
    let code_str = status_parts.next().ok_or("Missing HTTP status code")?;

    let status_code: u16 = parse_dec_u16(code_str)
        .ok_or("Invalid non-numeric HTTP status code")?;

    // 2. Parse Headers
    let mut content_length = 0usize;
    let mut is_chunked = false;

    for line in lines {
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim();
            let val = line[colon + 1..].trim();

            if eq_ignore_ascii_case(key, "Content-Length") {
                if let Some(len) = parse_dec_usize(val) {
                    content_length = len;
                }
            } else if eq_ignore_ascii_case(key, "Transfer-Encoding") {
                if val.to_ascii_lowercase().contains("chunked") {
                    is_chunked = true;
                }
            }
        }
    }

    let mut resp = HttpResponse::new();
    resp.status_code = status_code;
    resp.content_length = content_length;
    resp.is_chunked = is_chunked;

    // 3. Extract and unchunk body if needed
    if is_chunked {
        decode_chunked_body(body_bytes, &mut resp);
    } else {
        let copy_len = core::cmp::min(body_bytes.len(), MAX_HTTP_BODY_LEN);
        resp.body[..copy_len].copy_from_slice(&body_bytes[..copy_len]);
        resp.body_len = copy_len;
        if body_bytes.len() > MAX_HTTP_BODY_LEN {
            resp.is_truncated = true;
        }
    }

    Ok(resp)
}

/// Decodes chunked transfer encoding (`{hex_len}\r\n{data}\r\n...0\r\n\r\n`).
fn decode_chunked_body(raw: &[u8], resp: &mut HttpResponse) {
    let mut offset = 0;
    let mut out_len = 0;

    while offset < raw.len() {
        // Find CRLF for chunk size line
        let line_end = match raw[offset..].windows(2).position(|w| w == b"\r\n") {
            Some(pos) => offset + pos,
            None => break,
        };

        let size_str = match core::str::from_utf8(&raw[offset..line_end]) {
            Ok(s) => s.trim(),
            Err(_) => break,
        };

        let chunk_size = match parse_hex_usize(size_str) {
            Some(sz) => sz,
            None => break,
        };

        if chunk_size == 0 {
            break; // Terminal 0-length chunk reached
        }

        let chunk_start = line_end + 2;
        let chunk_end = chunk_start + chunk_size;
        if chunk_end > raw.len() {
            // Incomplete/truncated chunk
            let available = raw.len().saturating_sub(chunk_start);
            let to_copy = core::cmp::min(available, MAX_HTTP_BODY_LEN.saturating_sub(out_len));
            resp.body[out_len..out_len + to_copy].copy_from_slice(&raw[chunk_start..chunk_start + to_copy]);
            out_len += to_copy;
            resp.is_truncated = true;
            break;
        }

        let to_copy = core::cmp::min(chunk_size, MAX_HTTP_BODY_LEN.saturating_sub(out_len));
        resp.body[out_len..out_len + to_copy].copy_from_slice(&raw[chunk_start..chunk_start + to_copy]);
        out_len += to_copy;

        if chunk_size > to_copy {
            resp.is_truncated = true;
        }

        offset = chunk_end + 2; // Skip chunk data + trailing CRLF
    }

    resp.body_len = out_len;
}

fn parse_dec_u16(s: &str) -> Option<u16> {
    let mut val: u32 = 0;
    if s.is_empty() { return None; }
    for b in s.bytes() {
        if !b.is_ascii_digit() { return None; }
        val = val * 10 + (b - b'0') as u32;
        if val > 65535 { return None; }
    }
    Some(val as u16)
}

fn parse_dec_usize(s: &str) -> Option<usize> {
    let mut val: usize = 0;
    if s.is_empty() { return None; }
    for b in s.bytes() {
        if !b.is_ascii_digit() { return None; }
        val = val.checked_mul(10)?.checked_add((b - b'0') as usize)?;
    }
    Some(val)
}

fn parse_hex_usize(s: &str) -> Option<usize> {
    let mut val: usize = 0;
    let s = s.split(';').next()?.trim(); // Ignore chunk extensions if present
    if s.is_empty() { return None; }
    for b in s.bytes() {
        let digit = match b {
            b'0'..=b'9' => (b - b'0') as usize,
            b'a'..=b'f' => (b - b'a' + 10) as usize,
            b'A'..=b'F' => (b - b'A' + 10) as usize,
            _ => return None,
        };
        val = val.checked_mul(16)?.checked_add(digit)?;
    }
    Some(val)
}

fn eq_ignore_ascii_case(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false; }
    a.bytes().zip(b.bytes()).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}
