pub(crate) fn strip_ui_control_text(input: &str) -> String {
    input
        .lines()
        .filter(|line| !is_ui_control_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn is_ui_control_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_lowercase();
    if lower.contains("your browser does not support") {
        return true;
    }
    if trimmed.len() > 120 {
        return false;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    !tokens.is_empty() && tokens.iter().all(|t| is_ui_control_token(t))
}

fn is_ui_control_token(token: &str) -> bool {
    const UI_CONTROLS: &[&str] = &[
        "navigate_before",
        "navigate_next",
        "chevron_left",
        "chevron_right",
        "arrow_back",
        "arrow_forward",
        "arrow_upward",
        "arrow_downward",
        "arrow_drop_down",
        "arrow_drop_up",
        "arrow_left",
        "arrow_right",
        "expand_more",
        "expand_less",
        "unfold_more",
        "unfold_less",
        "first_page",
        "last_page",
        "more_horiz",
        "more_vert",
        "open_in_new",
        "open_in_full",
        "close_fullscreen",
        "fullscreen",
        "fullscreen_exit",
        "close",
        "search",
        "menu",
        "share",
        "\u{2190}",
        "\u{2192}",
        "\u{2191}",
        "\u{2193}",
        "\u{25B8}",
        "\u{25BE}",
        "\u{25C0}",
        "\u{25B6}",
        "\u{2913}",
        "\u{23F5}",
        "\u{203A}",
        "\u{2039}",
        "\u{00BB}",
        "\u{00AB}",
    ];
    UI_CONTROLS.contains(&token)
}
