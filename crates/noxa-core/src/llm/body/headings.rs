use std::collections::{BTreeMap, HashSet};

use once_cell::sync::Lazy;
use regex::Regex;

static HEADING_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(#{1,6})\s+(.+)$").unwrap());

pub(crate) fn dedup_heading_paragraph(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < lines.len() {
        if let Some(h_caps) = HEADING_RE.captures(lines[i].trim()) {
            let heading_text = h_caps.get(2).unwrap().as_str().trim();
            let heading_prefix = h_caps.get(1).unwrap().as_str();

            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }

            if j < lines.len() {
                let next_text = lines[j].trim();
                if !HEADING_RE.is_match(next_text) && text_is_duplicate(heading_text, next_text) {
                    let merged = if next_text.len() > heading_text.len() {
                        next_text
                    } else {
                        heading_text
                    };
                    out.push_str(&format!("{heading_prefix} {merged}\n"));
                    i = j + 1;
                    continue;
                }
            }
        }

        out.push_str(lines[i]);
        out.push('\n');
        i += 1;
    }

    out
}

fn text_is_duplicate(heading: &str, paragraph: &str) -> bool {
    let h = heading.to_lowercase();
    let p = paragraph.to_lowercase();
    h == p || p.starts_with(&h) || h.starts_with(&p)
}

pub(crate) fn dedup_text_against_headings(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();

    let heading_texts: HashSet<String> = lines
        .iter()
        .filter_map(|line| {
            HEADING_RE
                .captures(line.trim())
                .map(|caps| caps.get(2).unwrap().as_str().trim().to_lowercase())
        })
        .collect();

    if heading_texts.is_empty() {
        return input.to_string();
    }

    let mut out = String::with_capacity(input.len());

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || HEADING_RE.is_match(trimmed) {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if heading_texts.contains(&trimmed.to_lowercase()) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    out
}

pub(crate) fn dedup_duplicate_headings(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();

    let mut heading_positions: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(caps) = HEADING_RE.captures(line.trim()) {
            let level = caps.get(1).unwrap().as_str();
            let text = caps.get(2).unwrap().as_str().trim();
            let normalized = normalize_heading_key(text);
            if !normalized.is_empty() {
                let key = format!("{} {}", level, normalized);
                heading_positions.entry(key).or_default().push(i);
            }
        }
    }

    let mut skip: HashSet<usize> = HashSet::new();

    for positions in heading_positions.values() {
        if positions.len() < 2 {
            continue;
        }

        let first_idx = positions[0];
        let first_following = collect_following_content(&lines, first_idx);

        for &dup_idx in &positions[1..] {
            skip.insert(dup_idx);

            let dup_following = collect_following_content(&lines, dup_idx);
            for (offset, dup_line) in dup_following.iter().enumerate() {
                if offset < first_following.len()
                    && normalize_heading_key(dup_line)
                        == normalize_heading_key(&first_following[offset])
                {
                    let actual_idx = find_content_line_index(&lines, dup_idx, offset);
                    skip.insert(actual_idx);
                } else {
                    break;
                }
            }
        }
    }

    if skip.is_empty() {
        return input.to_string();
    }

    let mut out = String::with_capacity(input.len());
    for (i, line) in lines.iter().enumerate() {
        if skip.contains(&i) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    out
}

fn normalize_heading_key(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn collect_following_content(lines: &[&str], heading_idx: usize) -> Vec<String> {
    let mut content = Vec::new();
    let mut i = heading_idx + 1;
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() || HEADING_RE.is_match(trimmed) {
            break;
        }
        content.push(trimmed.to_string());
        i += 1;
    }
    content
}

fn find_content_line_index(lines: &[&str], heading_idx: usize, content_offset: usize) -> usize {
    let mut i = heading_idx + 1;
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    i + content_offset
}

pub(crate) fn strip_empty_headings(input: &str) -> String {
    let mut out = String::with_capacity(input.len());

    for line in input.lines() {
        if let Some(h_caps) = HEADING_RE.captures(line.trim()) {
            let heading_text = h_caps.get(2).unwrap().as_str().trim();
            if heading_text.is_empty()
                || heading_text.chars().all(|c| !c.is_alphanumeric())
                || is_noise_heading(heading_text)
            {
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }

    out
}

fn is_noise_heading(text: &str) -> bool {
    const NOISE: &[&str] = &["footer", "header", "navigation", "sidebar", "menu"];
    let lower = text.to_lowercase();
    NOISE.iter().any(|n| lower == *n)
}

pub(crate) fn strip_trailing_empty_headings(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut remove = vec![false; lines.len()];

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Also handle headings with no text (e.g. `##` with no space/content after).
        // HEADING_RE requires a space after the `#` characters, so these would be
        // skipped without the extra check below.
        let level = if !trimmed.is_empty() && trimmed.chars().all(|c| c == '#') {
            trimmed.len()
        } else if let Some(caps) = HEADING_RE.captures(trimmed) {
            caps.get(1).unwrap().as_str().len()
        } else {
            continue;
        };

        let mut next_content = None;
        for (j, line_j) in lines.iter().enumerate().skip(i + 1) {
            if !line_j.trim().is_empty() {
                next_content = Some(j);
                break;
            }
        }

        match next_content {
            None => remove[i] = true,
            Some(j) => {
                let next = lines[j].trim();
                let next_level = if !next.is_empty() && next.chars().all(|c| c == '#') {
                    Some(next.len())
                } else {
                    HEADING_RE
                        .captures(next)
                        .map(|c| c.get(1).unwrap().as_str().len())
                };
                if let Some(nl) = next_level
                    && nl <= level
                {
                    remove[i] = true;
                }
            }
        }
    }

    lines
        .iter()
        .enumerate()
        .filter(|(i, _)| !remove[*i])
        .map(|(_, line)| *line)
        .collect::<Vec<_>>()
        .join("\n")
}
