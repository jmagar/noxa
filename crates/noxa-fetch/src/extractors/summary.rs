pub fn markdown_from_title(title: &str, body: Option<&str>) -> String {
    match body.filter(|body| !body.trim().is_empty()) {
        Some(body) => format!("# {title}\n\n{body}"),
        None => format!("# {title}"),
    }
}
