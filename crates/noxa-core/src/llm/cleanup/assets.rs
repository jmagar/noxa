pub(crate) fn strip_asset_labels(input: &str) -> String {
    let mut in_code_block = false;
    input
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                return true;
            }
            if in_code_block {
                return true;
            }
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('>') {
                return true;
            }
            !is_asset_label(trimmed)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn is_asset_label(line: &str) -> bool {
    if line.starts_with('|') {
        return false;
    }
    if line.contains(" | ") {
        let segments: Vec<&str> = line.split(" | ").collect();
        let all_short = segments.iter().all(|s| s.len() < 40);
        let has_stat_numbers = segments.iter().any(|s| is_stat_text(s));
        if segments.len() >= 3 && all_short && !has_stat_numbers {
            return true;
        }
    }
    if line.contains(" > ") {
        let parts: Vec<&str> = line.split(" > ").collect();
        let has_asset_word = parts.iter().any(|p| {
            let lower = p.trim().to_lowercase();
            ["cover", "card", "image", "poster", "logo", "thumbnail"]
                .iter()
                .any(|kw| lower.contains(kw))
        });
        if has_asset_word && line.len() < 80 {
            return true;
        }
    }
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.len() >= 3 && words.len() <= 12 {
        let label_keywords = [
            "Art Card",
            "ArtCard",
            "Card Image",
            "Cover Image",
            "1x1",
            "SEO",
        ];
        if label_keywords.iter().any(|kw| line.contains(kw)) {
            return true;
        }
    }
    if !line.contains(' ') && line.contains('-') && line.len() > 10 && line.len() < 80 {
        let is_slug = line
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
        if is_slug {
            return true;
        }
    }
    false
}

fn is_stat_text(s: &str) -> bool {
    let s = s.trim();
    s.contains('%')
        || s.contains('#')
        || s.contains("M+")
        || s.contains("K+")
        || s.contains("B+")
        || s.contains("M ")
        || s.contains("K ")
        || s.contains("B ")
        || (s.ends_with('x') && s.chars().any(|c| c.is_ascii_digit()))
}
