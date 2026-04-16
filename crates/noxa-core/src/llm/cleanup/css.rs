use once_cell::sync::Lazy;
use regex::Regex;

use crate::noise;

pub(crate) fn strip_css_artifacts(input: &str) -> String {
    static CSS_INLINE_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"@(?:keyframes|font-face|media|supports|layer)\s*[^{]*\{[^}]*\}?").unwrap()
    });

    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        let trimmed = line.trim();
        if is_css_artifact_line(trimmed) {
            continue;
        }
        let cleaned = CSS_INLINE_RE.replace_all(line, "");
        let cleaned = cleaned.trim_end();
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(cleaned);
    }
    out
}

pub(crate) fn is_css_artifact_line(trimmed: &str) -> bool {
    !trimmed.is_empty()
        && trimmed.len() > 10
        && trimmed.contains('{')
        && trimmed.contains('}')
        && trimmed.contains(':')
        && !trimmed.contains(' ')
        && !trimmed.starts_with('#')
}

pub(crate) fn strip_css_class_lines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_code_block = false;

    for line in input.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
        }

        if in_code_block || trimmed.is_empty() {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if trimmed == "-" || trimmed == "*" || trimmed == "- " || trimmed == "* " {
            continue;
        }

        let is_structural = trimmed.starts_with('#')
            || trimmed.starts_with('>')
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ");

        if !is_structural && noise::is_css_class_text(trimmed) {
            continue;
        }

        if !is_structural {
            let cleaned = strip_trailing_css_classes(trimmed);
            if !cleaned.is_empty() {
                out.push_str(&cleaned);
                out.push('\n');
                continue;
            }
        }

        out.push_str(line);
        out.push('\n');
    }

    out
}

fn strip_trailing_css_classes(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.len() < 3 {
        return line.to_string();
    }

    let mut last_content = words.len();
    for i in (0..words.len()).rev() {
        if noise::is_css_class_word_pub(words[i]) {
            last_content = i;
        } else {
            break;
        }
    }

    if last_content < words.len() && words.len() - last_content >= 2 {
        words[..last_content].join(" ")
    } else {
        line.to_string()
    }
}
