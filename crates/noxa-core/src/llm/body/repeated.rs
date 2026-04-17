pub(super) fn dedup_repeated_phrases(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_code_block = false;

    for line in input.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if in_code_block || trimmed.is_empty() || trimmed.starts_with('#') {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let deduped = collapse_repeated_in_line(trimmed);
        out.push_str(&deduped);
        out.push('\n');
    }

    out
}

fn detect_long_line_cycle(words: &[&str]) -> Option<String> {
    for start in 0..=15.min(words.len().saturating_sub(100)) {
        let slice = &words[start..];
        if slice.len() < 100 {
            break;
        }

        for n_copies in (2..=5).rev() {
            if !slice.len().is_multiple_of(n_copies) {
                continue;
            }
            let cycle_len = slice.len() / n_copies;
            if cycle_len < 20 {
                continue;
            }
            let pattern = &slice[..cycle_len];
            if slice.chunks(cycle_len).all(|chunk| chunk == pattern) {
                let mut result: Vec<&str> = words[..start].to_vec();
                result.extend_from_slice(pattern);
                return Some(result.join(" "));
            }
        }

        for cycle_len in (30..=slice.len() / 2).rev() {
            let pattern = &slice[..cycle_len];
            let mut pos = cycle_len;
            let mut copies = 1;
            while pos + cycle_len <= slice.len() && &slice[pos..pos + cycle_len] == pattern {
                pos += cycle_len;
                copies += 1;
            }
            if copies >= 2 {
                let mut result: Vec<&str> = words[..start].to_vec();
                result.extend_from_slice(pattern);
                let remaining_start = start + pos;
                if remaining_start < words.len() {
                    result.extend_from_slice(&words[remaining_start..]);
                }
                return Some(result.join(" "));
            }
            if cycle_len < slice.len() / 2 - 50 {
                break;
            }
        }
    }

    None
}

pub(crate) fn collapse_repeated_in_line(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.len() < 4 {
        return line.to_string();
    }

    if words.len() >= 100
        && let Some(deduped) = detect_long_line_cycle(&words)
    {
        return deduped;
    }

    let mut result: Vec<&str> = Vec::with_capacity(words.len());
    let mut i = 0;
    let max_phrase = (words.len() / 2).min(20);

    while i < words.len() {
        let mut found_repeat = false;
        for phrase_len in (2..=max_phrase).rev() {
            if i + phrase_len * 2 > words.len() {
                continue;
            }
            let phrase = &words[i..i + phrase_len];
            let next = &words[i + phrase_len..i + phrase_len * 2];
            if phrase == next {
                result.extend_from_slice(phrase);
                let mut j = i + phrase_len;
                while j + phrase_len <= words.len() && &words[j..j + phrase_len] == phrase {
                    j += phrase_len;
                }
                i = j;
                found_repeat = true;
                break;
            }
        }
        if !found_repeat {
            result.push(words[i]);
            i += 1;
        }
    }

    result.join(" ")
}
