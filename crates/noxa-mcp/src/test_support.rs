use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub(crate) struct TestRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct TestResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl TestResponse {
    pub(crate) fn text(status: u16, body: impl Into<String>, content_type: &str) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), content_type.into())],
            body: body.into().into_bytes(),
        }
    }

    pub(crate) fn json(body: impl Into<String>) -> Self {
        Self::text(200, body, "application/json")
    }

    pub(crate) fn html(body: impl Into<String>) -> Self {
        Self::text(200, body, "text/html; charset=utf-8")
    }

    #[allow(dead_code)]
    pub(crate) fn redirect(location: impl Into<String>) -> Self {
        Self {
            status: 302,
            headers: vec![("Location".into(), location.into())],
            body: b"redirect".to_vec(),
        }
    }
}

pub(crate) struct TestHttpServer {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<TestRequest>>>,
    task: JoinHandle<()>,
}

impl TestHttpServer {
    pub(crate) async fn spawn<F>(handler: F) -> Self
    where
        F: Fn(TestRequest) -> TestResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let shared_requests = Arc::clone(&requests);
        let handler = Arc::new(handler);

        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _peer)) = listener.accept().await else {
                    break;
                };
                let handler = Arc::clone(&handler);
                let requests = Arc::clone(&shared_requests);
                tokio::spawn(async move {
                    let Ok(request) = read_request(&mut stream).await else {
                        return;
                    };
                    requests.lock().unwrap().push(request.clone());
                    let response = handler(request);
                    let _ = write_response(&mut stream, response).await;
                });
            }
        });

        Self {
            addr,
            requests,
            task,
        }
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    pub(crate) fn requests(&self) -> Vec<TestRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> std::io::Result<TestRequest> {
    let mut buffer = Vec::new();
    let mut header_end = None;

    while header_end.is_none() {
        let mut chunk = [0_u8; 1024];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        header_end = buffer.windows(4).position(|window| window == b"\r\n\r\n");
    }

    let header_end = header_end.map(|idx| idx + 4).unwrap_or(buffer.len());
    let header_bytes = &buffer[..header_end];
    let header_text = String::from_utf8_lossy(header_bytes);
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let path = request_parts.next().unwrap_or_default().to_string();

    let mut headers = HashMap::new();
    let mut content_length = 0_usize;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            let key = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if key == "content-length" {
                content_length = value.parse().unwrap_or(0);
            }
            headers.insert(key, value);
        }
    }

    let mut body = buffer[header_end..].to_vec();
    while body.len() < content_length {
        let mut chunk = vec![0_u8; content_length - body.len()];
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(TestRequest {
        method,
        path,
        headers,
        body,
    })
}

async fn write_response(
    stream: &mut tokio::net::TcpStream,
    response: TestResponse,
) -> std::io::Result<()> {
    let status_text = match response.status {
        200 => "OK",
        201 => "Created",
        302 => "Found",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };

    let mut raw = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        status_text,
        response.body.len()
    );
    for (name, value) in &response.headers {
        raw.push_str(name);
        raw.push_str(": ");
        raw.push_str(value);
        raw.push_str("\r\n");
    }
    raw.push_str("\r\n");

    stream.write_all(raw.as_bytes()).await?;
    stream.write_all(&response.body).await?;
    stream.shutdown().await
}
