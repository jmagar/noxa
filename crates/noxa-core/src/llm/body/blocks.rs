use std::collections::HashSet;

pub(crate) fn merge_stat_lines(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut in_code_block = false;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            out.push_str(lines[i]);
            out.push('\n');
            i += 1;
            continue;
        }

        if in_code_block {
            out.push_str(lines[i]);
            out.push('\n');
            i += 1;
            continue;
        }

        let len = trimmed.len();
        if len > 0 && len <= 25 && !is_structural_line(trimmed) {
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }

            if j > i + 1 && j < lines.len() {
                let next = lines[j].trim();
                if !next.is_empty() && !is_structural_line(next) && len + 1 + next.len() <= 120 {
                    out.push_str(trimmed);
                    out.push(' ');
                    out.push_str(next);
                    out.push('\n');
                    i = j + 1;
                    continue;
                }
            }
        }

        out.push_str(lines[i]);
        out.push('\n');
        i += 1;
    }

    out.trim().to_string()
}

fn is_structural_line(line: &str) -> bool {
    line.starts_with('#')
        || line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("```")
        || line.starts_with("> ")
}

const DEDUP_MIN_CHARS: usize = 20;
const DEDUP_PREFIX_WORDS: usize = 10;

fn normalize_fingerprint(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn prefix_fingerprint(normalized: &str) -> Option<String> {
    let words: Vec<&str> = normalized.split_whitespace().collect();
    if words.len() >= DEDUP_PREFIX_WORDS {
        Some(words[..DEDUP_PREFIX_WORDS].join(" "))
    } else {
        None
    }
}

pub(crate) fn dedup_content_blocks(input: &str) -> String {
    let blocks: Vec<&str> = input
        .split("\n\n")
        .filter(|b| !b.trim().is_empty())
        .collect();

    let mut seen_exact: HashSet<String> = HashSet::new();
    let mut seen_prefix: HashSet<String> = HashSet::new();
    let mut kept: Vec<String> = Vec::with_capacity(blocks.len());
    let mut in_code_block = false;

    for block in &blocks {
        let has_fence = block.lines().any(|l| l.trim_start().starts_with("```"));
        if in_code_block || has_fence {
            kept.push(block.to_string());
            for line in block.lines() {
                if line.trim_start().starts_with("```") {
                    in_code_block = !in_code_block;
                }
            }
            continue;
        }

        let trimmed = block.trim();
        if trimmed.len() < DEDUP_MIN_CHARS {
            kept.push(trimmed.to_string());
            continue;
        }
        if trimmed.lines().count() == 1 && is_structural_line(trimmed) {
            kept.push(trimmed.to_string());
            continue;
        }

        let fp = normalize_fingerprint(trimmed);
        if !seen_exact.insert(fp.clone()) {
            continue;
        }
        if let Some(pfp) = prefix_fingerprint(&fp)
            && !seen_prefix.insert(pfp)
        {
            continue;
        }
        kept.push(trimmed.to_string());
    }

    kept.join("\n\n")
}

pub(crate) fn dedup_lines(input: &str) -> String {
    let blocks: Vec<&str> = input.split("\n\n").collect();
    let mut out = Vec::with_capacity(blocks.len());
    let mut in_code_block = false;

    for block in blocks {
        let has_fence = block.lines().any(|l| l.trim_start().starts_with("```"));
        if in_code_block || has_fence {
            out.push(block.to_string());
            for line in block.lines() {
                if line.trim_start().starts_with("```") {
                    in_code_block = !in_code_block;
                }
            }
            continue;
        }

        let lines: Vec<&str> = block.lines().collect();
        if lines.len() <= 2 {
            out.push(block.to_string());
            continue;
        }

        let mut seen_exact: HashSet<String> = HashSet::new();
        let mut seen_prefix: HashSet<String> = HashSet::new();
        let mut kept: Vec<&str> = Vec::new();
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.len() < DEDUP_MIN_CHARS || is_structural_line(trimmed) {
                kept.push(line);
                continue;
            }
            let fp = normalize_fingerprint(trimmed);
            if !seen_exact.insert(fp.clone()) {
                continue;
            }
            if let Some(pfp) = prefix_fingerprint(&fp)
                && !seen_prefix.insert(pfp)
            {
                continue;
            }
            kept.push(line);
        }
        out.push(kept.join("\n"));
    }

    out.join("\n\n")
}

pub(crate) fn dedup_comma_lists(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let items: Vec<&str> = line.split(", ").map(|s| s.trim()).collect();
            if items.len() < 2 {
                return line.to_string();
            }

            if items.len() >= 6 {
                for cycle_len in 1..=items.len() / 2 {
                    if !items.len().is_multiple_of(cycle_len) {
                        continue;
                    }
                    let pattern = &items[..cycle_len];
                    let all_match = items.chunks(cycle_len).all(|chunk| chunk == pattern);
                    if all_match && items.len() / cycle_len >= 2 {
                        return pattern.join(", ");
                    }
                }
            }

            let mut deduped: Vec<&str> = Vec::with_capacity(items.len());
            for item in &items {
                if deduped
                    .last()
                    .is_none_or(|prev: &&str| !prev.eq_ignore_ascii_case(item))
                {
                    deduped.push(item);
                }
            }
            if deduped.len() < items.len() {
                return deduped.join(", ");
            }

            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn strip_empty_code_blocks(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut remove = vec![false; lines.len()];
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("```") {
            let mut j = i + 1;
            let mut all_blank = true;
            while j < lines.len() {
                if lines[j].trim().starts_with("```") {
                    break;
                }
                if !lines[j].trim().is_empty() {
                    all_blank = false;
                }
                j += 1;
            }
            if j < lines.len() && all_blank {
                for flag in &mut remove[i..=j] {
                    *flag = true;
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }

    lines
        .iter()
        .enumerate()
        .filter(|(i, _)| !remove[*i])
        .map(|(_, line)| *line)
        .collect::<Vec<_>>()
        .join("\n")
}
