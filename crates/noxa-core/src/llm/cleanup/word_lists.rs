pub(crate) fn collapse_word_lists(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        if !out.is_empty() {
            out.push('\n');
        }

        let trimmed = line.trim();
        if trimmed.len() < 200
            || trimmed.starts_with('#')
            || trimmed.starts_with('-')
            || trimmed.starts_with('>')
            || trimmed.starts_with('|')
            || trimmed.starts_with("```")
        {
            out.push_str(line);
            continue;
        }

        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if words.len() < 20 {
            out.push_str(line);
            continue;
        }

        if let Some(start_idx) = find_dump_start(&words) {
            let dump_len = words.len() - start_idx;
            if dump_len > 20 {
                let prose_part: Vec<&str> = words[..start_idx].to_vec();
                let dump_preview: Vec<&str> =
                    words[start_idx..start_idx + 3.min(dump_len)].to_vec();
                if prose_part.is_empty() {
                    out.push_str(&format!(
                        "{} ... and {} more",
                        dump_preview.join(" "),
                        dump_len - dump_preview.len()
                    ));
                } else {
                    out.push_str(&prose_part.join(" "));
                }
            } else {
                out.push_str(line);
            }
        } else {
            out.push_str(line);
        }
    }
    out
}

fn find_dump_start(words: &[&str]) -> Option<usize> {
    if words.len() < 25 {
        return None;
    }
    let window = 15;
    let mut consecutive_non_prose = 0;

    // Pre-compute prose classification to avoid repeated to_lowercase() in the
    // inner ratio scan, making the full function O(n) instead of O(n²).
    let is_prose: Vec<bool> = words
        .iter()
        .map(|w| is_prose_function_word(&w.to_lowercase()))
        .collect();
    // Prefix-sum of prose words for O(1) range queries
    let mut prose_prefix = vec![0usize; words.len() + 1];
    for i in 0..words.len() {
        prose_prefix[i + 1] = prose_prefix[i] + usize::from(is_prose[i]);
    }

    for (i, &prose) in is_prose.iter().enumerate() {
        if prose {
            consecutive_non_prose = 0;
        } else {
            consecutive_non_prose += 1;
            if consecutive_non_prose >= window {
                let start = i + 1 - window;
                let remaining_len = words.len() - start;
                let prose_in_remaining = prose_prefix[words.len()] - prose_prefix[start];
                let ratio = prose_in_remaining as f64 / remaining_len as f64;
                if ratio < 0.05 {
                    return Some(start);
                }
            }
        }
    }
    None
}

fn is_prose_function_word(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "a"
            | "an"
            | "of"
            | "to"
            | "for"
            | "with"
            | "in"
            | "on"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "being"
            | "and"
            | "but"
            | "or"
            | "not"
            | "that"
            | "this"
            | "these"
            | "it"
            | "its"
            | "you"
            | "your"
            | "we"
            | "our"
            | "they"
            | "from"
            | "by"
            | "at"
            | "as"
            | "if"
            | "so"
            | "no"
            | "can"
            | "will"
            | "has"
            | "have"
            | "had"
            | "do"
            | "does"
            | "did"
            | "about"
            | "into"
            | "than"
            | "then"
            | "also"
            | "more"
    )
}

pub(crate) fn dedup_adjacent_descriptions(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    if lines.len() < 3 {
        return input.to_string();
    }

    let mut out = String::with_capacity(input.len());
    let mut skip_next = false;

    for i in 0..lines.len() {
        if skip_next {
            skip_next = false;
            continue;
        }

        let current = lines[i].trim();

        if i + 1 < lines.len() {
            let next = lines[i + 1].trim();
            let next_lower = next.to_lowercase();
            // Case-insensitive "learn more" check covers all variants
            if let Some(_rest_lower) = next_lower.strip_prefix("learn more") {
                // Slice the original string past "learn more" (10 ASCII chars)
                let rest = next["learn more".len()..].trim().trim_start_matches('*').trim();
                if !rest.is_empty() && rest.len() > 15 && current.contains(rest) {
                    skip_next = true;
                }
            }
        }

        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(lines[i]);
    }
    out
}
