use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use noxa_store::domain_from_url;
use noxa_store::url_to_store_path;
use serde_json::json;
use tempfile::TempDir;

fn noxa_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_noxa"))
}

fn run_noxa(args: &[&str], current_dir: &Path, home_dir: &Path) -> std::process::Output {
    Command::new(noxa_bin())
        .args(args)
        .current_dir(current_dir)
        .env("HOME", home_dir)
        .env("NOXA_CONFIG", "/dev/null")
        .env("NOXA_LOG", "warn")
        .output()
        .expect("failed to run noxa")
}

fn run_noxa_with_status_dir(
    args: &[&str],
    current_dir: &Path,
    home_dir: &Path,
    status_dir: &Path,
) -> std::process::Output {
    Command::new(noxa_bin())
        .args(args)
        .current_dir(current_dir)
        .env("HOME", home_dir)
        .env("NOXA_CRAWL_STATUS_DIR", status_dir)
        .env("NOXA_CONFIG", "/dev/null")
        .env("NOXA_LOG", "warn")
        .output()
        .expect("failed to run noxa")
}

fn store_root(output_dir: &Path) -> PathBuf {
    output_dir.join(".noxa").join("content")
}

fn strip_ansi_codes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn write_doc(output_dir: &Path, url: &str, title: &str, markdown: &str) {
    let path = store_root(output_dir).join(url_to_store_path(url));
    fs::create_dir_all(path.parent().expect("doc path parent")).unwrap();
    fs::write(path.with_extension("md"), markdown).unwrap();
    fs::write(
        path.with_extension("json"),
        serde_json::to_vec_pretty(&json!({
            "url": url,
            "current": {
                "metadata": {
                    "url": url,
                    "title": title,
                    "word_count": markdown.split_whitespace().count(),
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

fn write_crawl_status(
    status_dir: &Path,
    url: &str,
    done: bool,
    pages_done: usize,
    total_words: usize,
) {
    fs::create_dir_all(status_dir).unwrap();
    let key = domain_from_url(url);
    fs::write(
        status_dir.join(format!("{key}.json")),
        serde_json::to_vec_pretty(&json!({
            "url": url,
            "pid": std::process::id(),
            "pages_done": pages_done,
            "pages_ok": pages_done,
            "pages_errors": 0,
            "max_pages": pages_done,
            "last_url": serde_json::Value::Null,
            "done": done,
            "elapsed_secs": 1.4,
            "docs_dir": status_dir.join("docs").display().to_string(),
            "excluded": 0,
            "total_words": total_words,
        }))
        .unwrap(),
    )
    .unwrap();
}

fn start_proxy_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let seen_clone = Arc::clone(&seen);

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            handle_proxy_connection(stream, &seen_clone);
        }
    });

    format!("http://{addr}")
}

fn handle_proxy_connection(mut stream: TcpStream, seen: &AtomicUsize) {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf).unwrap_or(0);
    if n == 0 {
        return;
    }

    let request = String::from_utf8_lossy(&buf[..n]);
    let first_line = request.lines().next().unwrap_or("");
    let target = first_line.split_whitespace().nth(1).unwrap_or("");
    let url =
        url::Url::parse(target).unwrap_or_else(|_| url::Url::parse("http://example.com/").unwrap());
    let body = match url.path() {
        "/" => {
            seen.fetch_add(1, Ordering::Relaxed);
            r#"<!doctype html><html><head><title>Root Page</title></head><body><main><h1>Root</h1><p>alpha beta gamma</p><a href="/docs/second">Second page</a></main></body></html>"#
        }
        "/docs/second" => {
            seen.fetch_add(1, Ordering::Relaxed);
            r#"<!doctype html><html><head><title>Second Page</title></head><body><main><p>delta epsilon zeta eta</p></main></body></html>"#
        }
        _ => "<html><body><main><p>not found</p></main></body></html>",
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

#[test]
fn list_root_and_domain_views_show_docs() {
    let sandbox = TempDir::new().unwrap();
    let output_dir = sandbox.path().join("output");
    let home_dir = TempDir::new().unwrap();

    write_doc(
        &output_dir,
        "https://example.com/docs/intro",
        "Intro Guide",
        "# Intro\n\nHello world for the intro doc.",
    );
    write_doc(
        &output_dir,
        "https://example.com/docs/api",
        "API Guide",
        "# API\n\nReference content.",
    );
    write_doc(
        &output_dir,
        "https://other.example.com/notes",
        "Other Notes",
        "# Notes\n\nOther domain content.",
    );

    let root = run_noxa(
        &[
            "--config",
            "/dev/null",
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--list",
        ],
        sandbox.path(),
        home_dir.path(),
    );
    let stderr = strip_ansi_codes(&String::from_utf8_lossy(&root.stderr));
    assert!(stderr.contains("stored docs"));
    assert!(stderr.contains("example_com  (2)"), "{stderr}");
    assert!(stderr.contains("other_example_com  (1)"), "{stderr}");

    let domain = run_noxa(
        &[
            "--config",
            "/dev/null",
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--list",
            "example.com",
        ],
        sandbox.path(),
        home_dir.path(),
    );
    let stderr = strip_ansi_codes(&String::from_utf8_lossy(&domain.stderr));
    assert!(stderr.contains("example.com"));
    assert!(stderr.contains("https://example.com/docs/api"));
    assert!(stderr.contains("https://example.com/docs/intro"));
    assert!(stderr.contains("docs/intro.md"));
    assert!(stderr.contains("docs/api.md"));
}

#[test]
fn retrieve_exact_and_fuzzy_query_return_expected_docs() {
    let sandbox = TempDir::new().unwrap();
    let output_dir = sandbox.path().join("output");
    let home_dir = TempDir::new().unwrap();

    write_doc(
        &output_dir,
        "https://example.com/docs/intro",
        "Intro Guide",
        "# Intro\n\nExact retrieval body.",
    );
    write_doc(
        &output_dir,
        "https://example.com/docs/appendix",
        "Intro Appendix",
        "# Appendix\n\nAppendix body.",
    );

    let exact = run_noxa(
        &[
            "--config",
            "/dev/null",
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--retrieve",
            "https://example.com/docs/intro",
        ],
        sandbox.path(),
        home_dir.path(),
    );
    let stdout = String::from_utf8_lossy(&exact.stdout);
    let stderr = String::from_utf8_lossy(&exact.stderr);
    assert!(stdout.contains("Exact retrieval body."));
    assert!(stderr.contains("retrieved"));
    assert!(stderr.contains("docs/intro.md"));

    let fuzzy = run_noxa(
        &[
            "--config",
            "/dev/null",
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--retrieve",
            "intro guide",
        ],
        sandbox.path(),
        home_dir.path(),
    );
    let stdout = String::from_utf8_lossy(&fuzzy.stdout);
    let stderr = String::from_utf8_lossy(&fuzzy.stderr);
    assert!(stderr.contains("best match"));
    assert!(stdout.contains("Exact retrieval body."));
}

#[test]
fn status_done_reads_json_fixture() {
    let sandbox = TempDir::new().unwrap();
    let output_dir = sandbox.path().join("output");
    let home_dir = TempDir::new().unwrap();
    let status_dir = sandbox.path().join("crawl-status");
    let url = format!("https://status-test-{}.example.com", std::process::id());
    let status_key = url_to_store_path(&url)
        .components()
        .next()
        .expect("status key")
        .as_os_str()
        .to_string_lossy()
        .to_string();
    let status_path = status_dir.join(format!("{status_key}.json"));
    write_crawl_status(&status_dir, &url, true, 4, 1234);
    assert!(
        status_path.exists(),
        "missing fixture at {}",
        status_path.display()
    );
    fs::create_dir_all(store_root(&output_dir).join("example_com")).unwrap();

    let output = run_noxa_with_status_dir(
        &[
            "--config",
            "/dev/null",
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--status",
            &url,
        ],
        sandbox.path(),
        home_dir.path(),
        &status_dir,
    );
    let stderr = strip_ansi_codes(&String::from_utf8_lossy(&output.stderr));
    assert!(stderr.contains("crawl"));
    assert!(stderr.contains("done"));
    assert!(stderr.contains("4 ok"));
    assert!(stderr.contains("~1k words"));
    assert!(stderr.contains(&format!(
        "noxa --list {}",
        url.trim_start_matches("https://")
    )));
}

#[test]
fn crawl_wait_streams_progress_and_writes_store() {
    let sandbox = TempDir::new().unwrap();
    let output_dir = sandbox.path().join("output");
    let home_dir = TempDir::new().unwrap();
    let status_dir = sandbox.path().join("crawl-status");
    let proxy = start_proxy_server();

    let output = run_noxa_with_status_dir(
        &[
            "--config",
            "/dev/null",
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--proxy",
            &proxy,
            "--crawl",
            "--wait",
            "--depth",
            "2",
            "--max-pages",
            "2",
            "--concurrency",
            "1",
            "--delay",
            "0",
            "http://example.com",
        ],
        sandbox.path(),
        home_dir.path(),
        &status_dir,
    );

    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = strip_ansi_codes(&String::from_utf8_lossy(&output.stderr));
    assert!(stderr.contains("[1/2] OK"), "{stderr}");
    assert!(stderr.contains("[2/2] OK"), "{stderr}");
    assert!(stderr.contains("✓"), "{stderr}");

    let root = store_root(&output_dir);
    assert!(root.join("example_com/index.md").exists());
    assert!(root.join("example_com/docs/second.md").exists());

    let crawl_status = status_dir.join("example_com.json");
    let status = fs::read_to_string(crawl_status).unwrap();
    assert!(status.contains(r#""done": true"#), "{status}");
    assert!(status.contains(r#""pages_done": 2"#), "{status}");
}
