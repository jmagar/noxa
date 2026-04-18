use once_cell::sync::Lazy;
use regex::Regex;

use crate::noise;

pub(crate) fn strip_css_artifacts(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        let trimmed = line.trim();
        if is_css_artifact_line(trimmed) {
            continue;
        }
        let cleaned = strip_css_at_rules(line);
        let cleaned = cleaned.trim_end().to_string();
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&cleaned);
    }
    out
}

/// Remove CSS at-rule blocks (`@keyframes`, `@font-face`, etc.) from a line,
/// handling nested braces so that `@media { .a { color: red; } }` is fully removed.
fn strip_css_at_rules(line: &str) -> String {
    static CSS_AT_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"@(?:keyframes|font-face|media|supports|layer)\s*[^{]*").unwrap());

    let mut result = line.to_string();
    // Iteratively remove at-rule blocks with balanced brace handling
    loop {
        let Some(m) = CSS_AT_RE.find(&result) else {
            break;
        };
        let start = m.start();
        // Find the matching closing brace after the at-rule header
        let after_header = m.end();
        let bytes = result.as_bytes();
        let Some(first_brace_offset) = bytes[after_header..].iter().position(|&b| b == b'{') else {
            // No opening brace — just remove the at-rule token
            result.replace_range(start..after_header, "");
            break;
        };
        let brace_start = after_header + first_brace_offset;
        let mut depth = 0usize;
        let mut end = result.len(); // default: consume to end of line if braces are unbalanced
        for (i, &b) in bytes[brace_start..].iter().enumerate() {
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        end = brace_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        result.replace_range(start..end, "");
    }
    result
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
                // Preserve original leading whitespace from `line`
                let leading = &line[..line.len() - line.trim_start().len()];
                out.push_str(leading);
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
