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
    for (i, word) in words.iter().enumerate() {
        if is_prose_function_word(&word.to_lowercase()) {
            consecutive_non_prose = 0;
        } else {
            consecutive_non_prose += 1;
            if consecutive_non_prose >= window {
                let start = i + 1 - window;
                let remaining = &words[start..];
                let prose_in_remaining = remaining
                    .iter()
                    .filter(|w| is_prose_function_word(&w.to_lowercase()))
                    .count();
                let ratio = prose_in_remaining as f64 / remaining.len() as f64;
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
            if let Some(rest) = next
                .strip_prefix("Learn more")
                .or_else(|| next.strip_prefix("LEARN MORE"))
                .or_else(|| next.strip_prefix("learn more"))
            {
                let rest = rest.trim().trim_start_matches('*').trim();
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
