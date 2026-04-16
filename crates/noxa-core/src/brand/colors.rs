use std::collections::HashMap;

use super::*;

/// Colors to filter out: pure white, pure black, and common grays.
fn is_boring_color(hex: &str) -> bool {
    matches!(
        hex,
        "#FFFFFF"
            | "#000000"
            | "#F8F8F8"
            | "#F5F5F5"
            | "#EEEEEE"
            | "#E5E5E5"
            | "#DDDDDD"
            | "#D4D4D4"
            | "#CCCCCC"
            | "#BBBBBB"
            | "#AAAAAA"
            | "#999999"
            | "#888888"
            | "#777777"
            | "#666666"
            | "#555555"
            | "#444444"
            | "#333333"
            | "#222222"
            | "#111111"
            | "#F0F0F0"
            | "#E0E0E0"
            | "#D0D0D0"
            | "#C0C0C0"
            | "#B0B0B0"
            | "#A0A0A0"
            | "#909090"
            | "#808080"
            | "#707070"
            | "#606060"
            | "#505050"
            | "#404040"
            | "#303030"
            | "#202020"
            | "#101010"
            | "#FAFAFA"
            | "#F9F9F9"
            | "#F7F7F7"
            | "#F4F4F4"
            | "#F3F3F3"
            | "#F2F2F2"
            | "#F1F1F1"
            | "#EFEFEF"
            | "#EBEBEB"
            | "#E8E8E8"
    )
}

pub(super) fn extract_colors(decls: &[css::CssDecl]) -> Vec<BrandColor> {
    let mut counts: HashMap<String, HashMap<ColorUsage, usize>> = HashMap::new();

    for decl in decls {
        let usage = classify_color_property(&decl.property);
        for hex in parse_colors_from_value(&decl.value) {
            if is_boring_color(&hex) {
                continue;
            }
            *counts
                .entry(hex)
                .or_default()
                .entry(usage.clone())
                .or_insert(0) += 1;
        }
    }

    let mut colors: Vec<BrandColor> = counts
        .into_iter()
        .map(|(hex, usage_map)| {
            let total: usize = usage_map.values().sum();
            let usage = usage_map
                .into_iter()
                .max_by_key(|(_, c)| *c)
                .map(|(u, _)| u)
                .unwrap_or(ColorUsage::Unknown);
            BrandColor {
                hex,
                usage,
                count: total,
            }
        })
        .collect();

    colors.sort_by(|a, b| b.count.cmp(&a.count));

    let mut assigned_primary = colors.iter().any(|c| c.usage == ColorUsage::Primary);
    let mut assigned_secondary = colors.iter().any(|c| c.usage == ColorUsage::Secondary);

    for color in &mut colors {
        if color.usage != ColorUsage::Unknown {
            continue;
        }
        if !assigned_primary {
            color.usage = ColorUsage::Primary;
            assigned_primary = true;
        } else if !assigned_secondary {
            color.usage = ColorUsage::Secondary;
            assigned_secondary = true;
        }
    }

    colors.truncate(10);
    colors
}

fn classify_color_property(property: &str) -> ColorUsage {
    match property {
        "background-color" | "background" => ColorUsage::Background,
        "color" => ColorUsage::Text,
        "border-color"
        | "border"
        | "border-top-color"
        | "border-bottom-color"
        | "border-left-color"
        | "border-right-color"
        | "outline-color" => ColorUsage::Accent,
        _ => ColorUsage::Unknown,
    }
}

fn parse_colors_from_value(value: &str) -> Vec<String> {
    let mut colors = Vec::new();

    for cap in HEX_COLOR.captures_iter(value) {
        if let Some(short) = cap.get(1) {
            colors.push(expand_short_hex(short.as_str()));
        } else if let Some(full) = cap.get(2) {
            colors.push(format!("#{}", full.as_str().to_ascii_uppercase()));
        }
    }

    for cap in RGB_COLOR.captures_iter(value) {
        let r: u8 = cap[1].parse().unwrap_or(0);
        let g: u8 = cap[2].parse().unwrap_or(0);
        let b: u8 = cap[3].parse().unwrap_or(0);
        colors.push(format!("#{:02X}{:02X}{:02X}", r, g, b));
    }

    for cap in RGBA_COLOR.captures_iter(value) {
        let r: u8 = cap[1].parse().unwrap_or(0);
        let g: u8 = cap[2].parse().unwrap_or(0);
        let b: u8 = cap[3].parse().unwrap_or(0);
        colors.push(format!("#{:02X}{:02X}{:02X}", r, g, b));
    }

    for cap in HSL_COLOR.captures_iter(value) {
        let h: f64 = cap[1].parse().unwrap_or(0.0);
        let s: f64 = cap[2].parse::<f64>().unwrap_or(0.0) / 100.0;
        let l: f64 = cap[3].parse::<f64>().unwrap_or(0.0) / 100.0;
        let (r, g, b) = hsl_to_rgb(h, s, l);
        colors.push(format!("#{:02X}{:02X}{:02X}", r, g, b));
    }

    colors
}

fn expand_short_hex(short: &str) -> String {
    let chars: Vec<char> = short.chars().collect();
    format!(
        "#{}{}{}{}{}{}",
        chars[0], chars[0], chars[1], chars[1], chars[2], chars[2]
    )
    .to_ascii_uppercase()
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let h_norm = h / 360.0;

    let r = hue_to_rgb(p, q, h_norm + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h_norm);
    let b = hue_to_rgb(p, q, h_norm - 1.0 / 3.0);

    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}
