use once_cell::sync::Lazy;
use regex::Regex;

pub(crate) fn decode_html_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }

    static ENTITY_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"&(#[xX][0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);").unwrap());

    ENTITY_RE
        .replace_all(input, |caps: &regex::Captures| {
            let entity = caps.get(1).unwrap().as_str();
            match entity {
                "nbsp" => " ".to_string(),
                "amp" => "&".to_string(),
                "lt" => "<".to_string(),
                "gt" => ">".to_string(),
                "quot" => "\"".to_string(),
                "apos" => "'".to_string(),
                "mdash" => "\u{2014}".to_string(),
                "ndash" => "\u{2013}".to_string(),
                "laquo" => "\u{00AB}".to_string(),
                "raquo" => "\u{00BB}".to_string(),
                "copy" => "\u{00A9}".to_string(),
                "reg" => "\u{00AE}".to_string(),
                "trade" => "\u{2122}".to_string(),
                "hellip" => "\u{2026}".to_string(),
                "bull" => "\u{2022}".to_string(),
                s if s.starts_with("#x") || s.starts_with("#X") => u32::from_str_radix(&s[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| caps[0].to_string()),
                s if s.starts_with('#') => s[1..]
                    .parse::<u32>()
                    .ok()
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| caps[0].to_string()),
                _ => caps[0].to_string(),
            }
        })
        .into_owned()
}

pub(crate) fn strip_invisible_unicode(input: &str) -> String {
    if !input.chars().any(|c| matches!(
        c,
        '\u{200B}'
            | '\u{200C}'
            | '\u{200D}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{FEFF}'
            | '\u{00AD}'
            | '\u{2060}'
            | '\u{2062}'
            | '\u{2063}'
            | '\u{2064}'
            | '\u{034F}'
    )) {
        return input.to_string();
    }

    input
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{200B}'
                    | '\u{200C}'
                    | '\u{200D}'
                    | '\u{200E}'
                    | '\u{200F}'
                    | '\u{FEFF}'
                    | '\u{00AD}'
                    | '\u{2060}'
                    | '\u{2062}'
                    | '\u{2063}'
                    | '\u{2064}'
                    | '\u{034F}'
            )
        })
        .collect()
}

pub(crate) fn strip_leaked_js(input: &str) -> String {
    static JS_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"self\.__\w+").unwrap());

    let mut out = String::with_capacity(input.len());
    let mut in_code_fence = false;

    for line in input.lines() {
        if line.trim().starts_with("```") {
            in_code_fence = !in_code_fence;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_code_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if JS_PATTERN.is_match(line) {
            if let Some(idx) = line.find("self.__") {
                let cleaned = line[..idx].trim_end();
                if !cleaned.is_empty() {
                    out.push_str(cleaned);
                    out.push('\n');
                }
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    if !input.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    out
}

pub(crate) fn collapse_spaced_text(input: &str) -> String {
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

        if in_code_block || trimmed.is_empty() {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let collapsed = collapse_spaced_segments(trimmed);
        out.push_str(&collapsed);
        out.push('\n');
    }

    if !input.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    out
}

fn collapse_spaced_segments(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() < 7 {
        return line.to_string();
    }

    let mut result = String::with_capacity(line.len());
    let mut i = 0;

    while i < chars.len() {
        if !chars[i].is_whitespace() {
            let seg_start = i;
            let mut real_chars: Vec<char> = vec![chars[i]];
            let mut j = i + 1;

            while j + 1 < chars.len() && chars[j] == ' ' && !chars[j + 1].is_whitespace() {
                real_chars.push(chars[j + 1]);
                j += 2;
            }

            if real_chars.len() >= 4 {
                let starts_ok = seg_start == 0 || chars[seg_start - 1].is_whitespace();
                let ends_ok = j >= chars.len() || chars[j].is_whitespace();

                if starts_ok && ends_ok {
                    let mut collapsed = String::with_capacity(real_chars.len() + 4);
                    for (idx, &ch) in real_chars.iter().enumerate() {
                        if idx > 0 && ch.is_uppercase() && real_chars[idx - 1].is_lowercase() {
                            collapsed.push(' ');
                        }
                        collapsed.push(ch);
                    }
                    result.push_str(&collapsed);
                    i = j;
                    continue;
                }
            }
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

pub(crate) fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut consecutive_blanks = 0;

    for line in input.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            consecutive_blanks += 1;
            if consecutive_blanks <= 1 {
                out.push('\n');
            }
        } else {
            consecutive_blanks = 0;
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    out.trim().to_string()
}

// Bold: **text** or __text__.
// Inner content must not be whitespace-only and must not start/end with a space.
// This avoids stripping "* list item *" patterns where spaces border the asterisks.
static BOLD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\*\*([^\s*][^*\n]*[^\s*]|[^\s*])\*\*|__([^\s_][^_\n]*[^\s_]|[^\s_])__").unwrap()
});
// Italic *text* — inner must not start/end with space or asterisk; and opening *
// must not be immediately followed by another * (already handled by BOLD_RE running first).
static ITALIC_STAR_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\*([^\s*][^*\n]*[^\s*]|[^\s*])\*").unwrap()
});
// Italic _text_ — keep existing word-boundary requirement
static ITALIC_UNDER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b_([^_\n]+)_\b").unwrap());

pub(crate) fn strip_emphasis(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_code_block = false;

    for line in input.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if in_code_block {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let s = BOLD_RE.replace_all(line, |caps: &regex::Captures| {
            caps.get(1)
                .or_else(|| caps.get(2))
                .map_or("", |m| m.as_str())
                .to_string()
        });
        let s = ITALIC_STAR_RE.replace_all(&s, "$1");
        let s = ITALIC_UNDER_RE.replace_all(&s, "$1");
        out.push_str(&s);
        out.push('\n');
    }

    if !input.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    out
}
