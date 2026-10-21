//! The brain's voice: a small open-source language model running locally in [Ollama](https://ollama.com).
//!
//! Free, offline after the first download, and private: nothing leaves the computer.
//! Ollama listens on `127.0.0.1:11434`; this module speaks its HTTP API with nothing but the
//! standard library (no TLS needed for localhost), so it adds no dependencies.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const OLLAMA_ADDR: &str = "127.0.0.1:11434";

/// Voices to choose from: free models with permissive licenses (Apache 2.0).
pub const MODELS: [(&str, &str); 3] = [("qwen3:0.6b", "0.5 GB"), ("qwen3:1.7b", "1.4 GB"), ("qwen3:4b", "2.5 GB")];
pub const DEFAULT_MODEL: &str = "qwen3:1.7b";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    /// Ollama is installed and running.
    pub running: bool,
    pub version: Option<String>,
    /// Models already downloaded.
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullProgress {
    pub status: String,
    pub completed: u64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// `system`, `user` or `assistant`.
    pub role: String,
    pub content: String,
}

/// Is Ollama running, and which models does it have?
pub fn status(addr: &str) -> VoiceStatus {
    let get = |path: &str| -> Option<Value> {
        let mut body = String::new();
        request(addr, "GET", path, None, Duration::from_millis(800)).ok()?.body.read_to_string(&mut body).ok()?;
        serde_json::from_str(&body).ok()
    };
    let Some(version) = get("/api/version") else {
        return VoiceStatus { running: false, version: None, models: Vec::new() };
    };
    let models = get("/api/tags")
        .and_then(|tags| {
            tags["models"]
                .as_array()
                .map(|list| list.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect())
        })
        .unwrap_or_default();
    VoiceStatus { running: true, version: version["version"].as_str().map(String::from), models }
}

/// Downloads a model. Calls `on_progress` as bytes arrive.
pub fn pull(addr: &str, model: &str, mut on_progress: impl FnMut(PullProgress)) -> Result<(), String> {
    let body = json!({ "model": model, "stream": true }).to_string();
    let response = request(addr, "POST", "/api/pull", Some(&body), Duration::from_secs(120)).map_err(not_running)?;
    for line in response.lines() {
        let event = line?;
        on_progress(PullProgress {
            status: event["status"].as_str().unwrap_or("").to_string(),
            completed: event["completed"].as_u64().unwrap_or(0),
            total: event["total"].as_u64().unwrap_or(0),
        });
    }
    Ok(())
}

/// Sends a conversation and streams the answer word by word into `on_token`.
/// Returns the whole answer. Setting `stop` ends the answer early.
pub fn chat(
    addr: &str,
    model: &str,
    messages: &[ChatMessage],
    stop: &AtomicBool,
    mut on_token: impl FnMut(&str),
) -> Result<String, String> {
    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        // Qwen3 can "think out loud" first; for short answers we want it to just talk.
        "think": false,
        "options": { "temperature": 0.4, "num_ctx": 8192 },
    });
    let response = match request(addr, "POST", "/api/chat", Some(&body.to_string()), Duration::from_secs(300)) {
        Ok(r) if r.status == 400 => {
            // Older Ollama or a model without a thinking switch: try again without it.
            body.as_object_mut().map(|o| o.remove("think"));
            request(addr, "POST", "/api/chat", Some(&body.to_string()), Duration::from_secs(300))
                .map_err(not_running)?
        }
        other => other.map_err(not_running)?,
    };
    let mut filter = ThinkFilter::default();
    let mut answer = String::new();
    for line in response.lines() {
        let event = line?;
        if let Some(piece) = event["message"]["content"].as_str() {
            let visible = filter.push(piece);
            if !visible.is_empty() {
                answer.push_str(&visible);
                on_token(&visible);
            }
        }
        if stop.load(Ordering::Relaxed) || event["done"].as_bool() == Some(true) {
            break;
        }
    }
    Ok(answer.trim().to_string())
}

fn not_running(e: io::Error) -> String {
    format!("ollama_unreachable: {e}")
}

/// Hides `<think>...</think>` blocks some models emit before the answer.
#[derive(Default)]
struct ThinkFilter {
    inside: bool,
    pending: String,
}

impl ThinkFilter {
    fn push(&mut self, piece: &str) -> String {
        self.pending.push_str(piece);
        let mut out = String::new();
        loop {
            let tag = if self.inside { "</think>" } else { "<think>" };
            if let Some(at) = self.pending.find(tag) {
                if !self.inside {
                    out.push_str(&self.pending[..at]);
                }
                self.pending.drain(..at + tag.len());
                self.inside = !self.inside;
                continue;
            }
            // Keep a possible partial tag for the next piece.
            let keep = (1..tag.len()).rev().find(|&n| self.pending.ends_with(&tag[..n])).unwrap_or(0);
            let cut = self.pending.len() - keep;
            if !self.inside {
                out.push_str(&self.pending[..cut]);
            }
            self.pending.drain(..cut);
            return out;
        }
    }
}

// ---------- a tiny HTTP/1.1 client ----------

struct Response {
    status: u16,
    body: Box<dyn Read>,
}

impl Response {
    /// The body as newline-delimited JSON (how Ollama streams). Error objects become `Err`.
    fn lines(self) -> impl Iterator<Item = Result<Value, String>> {
        let status = self.status;
        BufReader::new(self.body).lines().map_while(Result::ok).filter(|l| !l.trim().is_empty()).map(move |line| {
            let value: Value = serde_json::from_str(&line).map_err(|e| format!("bad reply from Ollama: {e}"))?;
            match value["error"].as_str() {
                Some(error) => Err(error.to_string()),
                None if status >= 400 => Err(format!("HTTP {status}")),
                None => Ok(value),
            }
        })
    }
}

fn request(addr: &str, method: &str, path: &str, body: Option<&str>, timeout: Duration) -> io::Result<Response> {
    let socket: SocketAddr = addr.parse().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad address"))?;
    let mut stream = TcpStream::connect_timeout(&socket, Duration::from_millis(800))?;
    stream.set_read_timeout(Some(timeout))?;
    let body = body.unwrap_or("");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let status = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let (mut chunked, mut length) = (false, None);
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
            chunked = true;
        } else if let Some(value) = lower.strip_prefix("content-length:") {
            length = value.trim().parse::<u64>().ok();
        }
    }
    let body: Box<dyn Read> = match (chunked, length) {
        (true, _) => Box::new(Chunked { inner: reader, left: 0, done: false }),
        (false, Some(n)) => Box::new(reader.take(n)),
        (false, None) => Box::new(reader),
    };
    Ok(Response { status, body })
}

/// Decodes `Transfer-Encoding: chunked`: `<hex size>\r\n<bytes>\r\n` ... `0\r\n\r\n`.
struct Chunked<R: BufRead> {
    inner: R,
    left: usize,
    done: bool,
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done {
            return Ok(0);
        }
        if self.left == 0 {
            let mut size = String::new();
            self.inner.read_line(&mut size)?;
            if size.trim().is_empty() {
                // the CRLF that ends the previous chunk
                size.clear();
                self.inner.read_line(&mut size)?;
            }
            let hex = size.trim().split(';').next().unwrap_or("0");
            self.left =
                usize::from_str_radix(hex, 16).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad chunk"))?;
            if self.left == 0 {
                self.done = true;
                return Ok(0);
            }
        }
        let max = buf.len().min(self.left);
        let n = self.inner.read(&mut buf[..max])?;
        self.left -= n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    /// A pretend Ollama: answers each connection with the given (path prefix -> response) rules.
    fn fake_ollama(routes: Vec<(&'static str, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if let Some(v) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap();
                    }
                    if header.trim().is_empty() {
                        break;
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
                let reply = routes.iter().find(|(p, _)| path.starts_with(p)).map(|(_, r)| r.clone());
                let mut stream = stream;
                let _ = stream.write_all(
                    reply.unwrap_or_else(|| "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".into()).as_bytes(),
                );
            }
        });
        addr
    }

    fn chunked(lines: &[&str]) -> String {
        let mut out =
            String::from("HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\n\r\n");
        for line in lines {
            let data = format!("{line}\n");
            out += &format!("{:x}\r\n{data}\r\n", data.len());
        }
        out + "0\r\n\r\n"
    }

    fn json_reply(body: &str) -> String {
        format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len())
    }

    #[test]
    fn status_when_running_and_when_not() {
        let addr = fake_ollama(vec![
            ("/api/version", json_reply(r#"{"version":"0.12.3"}"#)),
            ("/api/tags", json_reply(r#"{"models":[{"name":"qwen3:1.7b"},{"name":"llama3.2:1b"}]}"#)),
        ]);
        let s = status(&addr);
        assert!(s.running);
        assert_eq!(s.version.as_deref(), Some("0.12.3"));
        assert_eq!(s.models, ["qwen3:1.7b", "llama3.2:1b"]);

        let nobody = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().to_string(); // closed when dropped
        assert!(!status(&nobody).running);
    }

    #[test]
    fn chat_streams_tokens_and_hides_thinking() {
        let addr = fake_ollama(vec![(
            "/api/chat",
            chunked(&[
                r#"{"message":{"role":"assistant","content":"<thi"},"done":false}"#,
                r#"{"message":{"role":"assistant","content":"nk>hmm</think>Удали "},"done":false}"#,
                r#"{"message":{"role":"assistant","content":"node_modules."},"done":false}"#,
                r#"{"message":{"role":"assistant","content":""},"done":true}"#,
            ]),
        )]);
        let mut streamed = String::new();
        let messages = [ChatMessage { role: "user".into(), content: "что удалить?".into() }];
        let answer = chat(&addr, "qwen3:1.7b", &messages, &AtomicBool::new(false), |t| streamed.push_str(t)).unwrap();
        assert_eq!(answer, "Удали node_modules.");
        assert_eq!(streamed, "Удали node_modules.");
    }

    #[test]
    fn errors_are_reported() {
        let addr = fake_ollama(vec![(
            "/api/chat",
            "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: 39\r\n\r\n{\"error\":\"model 'qwen3:9b' not found\"}\n"
                .into(),
        )]);
        let err = chat(&addr, "qwen3:9b", &[], &AtomicBool::new(false), |_| {}).unwrap_err();
        assert!(err.contains("not found"), "{err}");
    }

    #[test]
    fn pull_reports_progress() {
        let addr = fake_ollama(vec![(
            "/api/pull",
            chunked(&[
                r#"{"status":"pulling manifest"}"#,
                r#"{"status":"downloading","digest":"sha256:x","total":100,"completed":40}"#,
                r#"{"status":"success"}"#,
            ]),
        )]);
        let mut events = Vec::new();
        pull(&addr, "qwen3:0.6b", |p| events.push((p.status, p.completed, p.total))).unwrap();
        assert_eq!(events[1], ("downloading".to_string(), 40, 100));
        assert_eq!(events.last().unwrap().0, "success");
    }

    #[test]
    fn think_filter_handles_split_tags() {
        let mut f = ThinkFilter::default();
        let out: String = ["Hi <", "think>secret</th", "ink> there"].iter().map(|p| f.push(p)).collect();
        assert_eq!(out, "Hi  there");
    }
}
