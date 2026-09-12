//! Small HTTP/1 read-only host: httparse framing, one request per connection,
//! fixed workers, bounded headers and read/write deadlines.
use crate::catalog::{Published, Query};
use arena_runner::tournament::monitor::{MAX_FILE_BYTES, guarded_path};
use std::{
    fs,
    io::{self, Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

pub struct Host {
    pub assets: PathBuf,
    pub authority: String,
    pub token: String,
    pub published: Arc<RwLock<Published>>,
}
struct Response {
    code: u16,
    mime: &'static str,
    bytes: Vec<u8>,
}
impl Response {
    fn text(code: u16, text: impl AsRef<str>) -> Self {
        Self {
            code,
            mime: "text/plain; charset=utf-8",
            bytes: text.as_ref().as_bytes().to_vec(),
        }
    }
    fn json(json: arena_runner::tournament::archive::Json) -> Self {
        Self {
            code: 200,
            mime: "application/json; charset=utf-8",
            bytes: json.to_json().into_bytes(),
        }
    }
}
impl Host {
    pub fn connection(&self, mut stream: TcpStream) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut buffer = [0u8; 16 * 1024];
        let mut used = 0;
        let response = loop {
            let mut headers = [httparse::EMPTY_HEADER; 64];
            let mut request = httparse::Request::new(&mut headers);
            match request.parse(&buffer[..used]) {
                Ok(httparse::Status::Complete(_)) => break self.route(&request),
                Err(_) => break Response::text(400, "invalid HTTP request"),
                Ok(httparse::Status::Partial) => {}
            }
            if used == buffer.len() {
                break Response::text(431, "request headers too large");
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break Response::text(408, "request timeout");
            }
            stream.set_read_timeout(Some(remaining))?;
            match stream.read(&mut buffer[used..]) {
                Ok(0) => return Ok(()),
                Ok(n) => used += n,
                Err(_) => break Response::text(408, "request timeout"),
            }
        };
        let headers = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCross-Origin-Resource-Policy: same-origin\r\nX-Frame-Options: DENY\r\n\r\n",
            response.code,
            if response.code == 200 { "OK" } else { "Error" },
            response.mime,
            response.bytes.len()
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        for bytes in [headers.as_bytes(), &response.bytes] {
            let mut pending = bytes;
            while !pending.is_empty() {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "response timeout"));
                }
                stream.set_write_timeout(Some(remaining))?;
                let n = stream.write(&pending[..pending.len().min(64 * 1024)])?;
                if n == 0 {
                    return Err(io::ErrorKind::WriteZero.into());
                }
                pending = &pending[n..];
            }
        }
        Ok(())
    }
    fn route(&self, request: &httparse::Request<'_, '_>) -> Response {
        let header = |name: &str| -> Option<&str> {
            let mut values = request
                .headers
                .iter()
                .filter(|h| h.name.eq_ignore_ascii_case(name));
            let first = std::str::from_utf8(values.next()?.value).ok()?;
            if values.next().is_some() {
                None
            } else {
                Some(first)
            }
        };
        if header("Host") != Some(&self.authority) {
            return Response::text(403, "Host rejected; use the printed URL");
        }
        if request
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("Origin"))
            && header("Origin") != Some(format!("http://{}", self.authority).as_str())
        {
            return Response::text(403, "Origin rejected");
        }
        if header("Sec-Fetch-Site") == Some("cross-site") {
            return Response::text(403, "cross-site request rejected");
        }
        if request.method != Some("GET") {
            return Response::text(405, "read-only: GET required");
        }
        if request.headers.iter().any(|h| {
            h.name.eq_ignore_ascii_case("Transfer-Encoding")
                || h.name.eq_ignore_ascii_case("Content-Length")
        }) {
            return Response::text(400, "request bodies are not supported");
        }
        let (path, raw_query) = request
            .path
            .unwrap_or("")
            .split_once('?')
            .unwrap_or((request.path.unwrap_or(""), ""));
        if path.contains(['%', '\\']) || path.split('/').any(|s| s == "." || s == "..") {
            return Response::text(400, "invalid path");
        }
        if path.starts_with("/api/") {
            if header("Authorization") != Some(format!("Bearer {}", self.token).as_str()) {
                return Response::text(
                    401,
                    "open the URL printed by arena-viewer to authorize this tab",
                );
            }
            let snapshot = self.published.read().unwrap().clone();
            if path == "/api/events" {
                return Response::json(snapshot.list());
            }
            let parts: Vec<_> = path.split('/').collect();
            if parts.len() >= 4 && parts[1..3] == ["api", "events"] {
                if let Some(event) = snapshot.events.iter().find(|e| e.id == parts[3]) {
                    if parts.len() == 4 {
                        return match Query::parse(raw_query) {
                            Ok(q) => Response::json(event.detail(&q)),
                            Err(e) => Response::text(400, e),
                        };
                    }
                    if parts.len() == 9
                        && parts[4] == "matches"
                        && parts[6] == "attempts"
                        && parts[8] == "replay"
                    {
                        let Ok(index) = parts[5].parse() else {
                            return Response::text(400, "invalid match index");
                        };
                        return match event.replay(index, parts[7]) {
                            Ok(bytes) => Response {
                                code: 200,
                                mime: "application/octet-stream",
                                bytes,
                            },
                            Err(e) => Response::text(409, e.to_string()),
                        };
                    }
                }
            }
            return Response::text(404, "unknown tournament resource");
        }
        let relative = if matches!(path, "/" | "/index.html") {
            "index.html"
        } else if let Some(p) = path.strip_prefix("/assets/") {
            if p.is_empty() {
                return Response::text(404, "not found");
            }
            &path[1..]
        } else {
            return Response::text(404, "not found");
        };
        let mime = match Path::new(relative).extension().and_then(|s| s.to_str()) {
            Some("html") if relative == "index.html" => "text/html; charset=utf-8",
            Some("js") => "text/javascript; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            Some("wasm") => "application/wasm",
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            _ => return Response::text(404, "asset type not exposed"),
        };
        let read = || -> io::Result<Vec<u8>> {
            let p = guarded_path(&self.assets, Path::new(relative))?;
            let file = fs::File::open(p)?;
            if !file.metadata()?.is_file() {
                return Err(io::Error::other("not a regular file"));
            }
            let mut data = Vec::new();
            file.take(MAX_FILE_BYTES + 1).read_to_end(&mut data)?;
            if data.len() as u64 > MAX_FILE_BYTES {
                return Err(io::Error::other("asset too large"));
            }
            Ok(data)
        };
        match read() {
            Ok(bytes) => Response {
                code: 200,
                mime,
                bytes,
            },
            Err(_) => Response::text(404, "asset unavailable"),
        }
    }
}
