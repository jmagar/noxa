use super::*;

#[test]
fn test_hex_colors() {
    let html = r#"<html><head><style>
        .header { background-color: #3498db; }
        .text { color: #2c3e50; }
        .border { border-color: #e74c3c; }
    </style></head><body></body></html>"#;

    let brand = extract_brand(html, None);
    assert!(!brand.colors.is_empty(), "should extract colors");

    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(hexes.contains(&"#3498DB"), "should find header bg color");
    assert!(hexes.contains(&"#2C3E50"), "should find text color");
    assert!(hexes.contains(&"#E74C3C"), "should find border color");

    // Verify usage classification
    let bg = brand.colors.iter().find(|c| c.hex == "#3498DB").unwrap();
    assert_eq!(bg.usage, ColorUsage::Background);

    let text = brand.colors.iter().find(|c| c.hex == "#2C3E50").unwrap();
    assert_eq!(text.usage, ColorUsage::Text);

    let accent = brand.colors.iter().find(|c| c.hex == "#E74C3C").unwrap();
    assert_eq!(accent.usage, ColorUsage::Accent);
}

#[test]
fn test_short_hex_expansion() {
    let html = r#"<html><head><style>
        .x { color: #f00; }
        .y { background-color: #0af; }
    </style></head><body></body></html>"#;

    let brand = extract_brand(html, None);
    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(hexes.contains(&"#FF0000"), "#f00 should expand to #FF0000");
    assert!(hexes.contains(&"#00AAFF"), "#0af should expand to #00AAFF");
}

#[test]
fn test_rgb_colors() {
    let html = r#"<html><head><style>
        .btn { background-color: rgb(52, 152, 219); }
        .link { color: rgba(231, 76, 60, 0.8); }
    </style></head><body></body></html>"#;

    let brand = extract_brand(html, None);
    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(hexes.contains(&"#3498DB"), "rgb(52,152,219) -> #3498DB");
    assert!(hexes.contains(&"#E74C3C"), "rgba(231,76,60,0.8) -> #E74C3C");
}

#[test]
fn test_hsl_colors() {
    let html = r#"<html><head><style>
        .x { color: hsl(0, 100%, 50%); }
        .y { background-color: hsla(240, 100%, 50%, 0.5); }
    </style></head><body></body></html>"#;

    let brand = extract_brand(html, None);
    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(hexes.contains(&"#FF0000"), "hsl(0,100%,50%) -> #FF0000");
    assert!(hexes.contains(&"#0000FF"), "hsla(240,100%,50%) -> #0000FF");
}

#[test]
fn test_boring_colors_filtered() {
    let html = r#"<html><head><style>
        body { background-color: #ffffff; color: #000000; }
        .gray { color: #cccccc; }
        .brand { color: #3498db; }
    </style></head><body></body></html>"#;

    let brand = extract_brand(html, None);
    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(!hexes.contains(&"#FFFFFF"), "white should be filtered");
    assert!(!hexes.contains(&"#000000"), "black should be filtered");
    assert!(
        !hexes.contains(&"#CCCCCC"),
        "common gray should be filtered"
    );
    assert!(hexes.contains(&"#3498DB"), "brand color should survive");
}

#[test]
fn test_font_extraction() {
    let html = r#"<html><head><style>
        body { font-family: "Inter", "Helvetica Neue", sans-serif; }
        code { font-family: 'Fira Code', monospace; }
        h1 { font-family: Inter, sans-serif; }
    </style></head><body></body></html>"#;

    let brand = extract_brand(html, None);
    assert!(
        brand.fonts.contains(&"Inter".to_string()),
        "should find Inter"
    );
    assert!(
        brand.fonts.contains(&"Helvetica Neue".to_string()),
        "should find Helvetica Neue"
    );
    assert!(
        brand.fonts.contains(&"Fira Code".to_string()),
        "should find Fira Code"
    );
    // Generic families should be excluded
    assert!(!brand.fonts.contains(&"sans-serif".to_string()));
    assert!(!brand.fonts.contains(&"monospace".to_string()));
}

#[test]
fn test_font_ordering_by_frequency() {
    let html = r#"<html><head><style>
        body { font-family: "Inter", sans-serif; }
        p { font-family: "Inter", sans-serif; }
        h1 { font-family: "Inter", sans-serif; }
        code { font-family: "Fira Code", monospace; }
    </style></head><body></body></html>"#;

    let brand = extract_brand(html, None);
    assert!(!brand.fonts.is_empty());
    assert_eq!(
        brand.fonts[0], "Inter",
        "most frequent font should be first"
    );
}

#[test]
fn test_logo_by_class() {
    let html = r#"<html><body>
        <header>
            <img class="site-logo" src="/images/logo.png" alt="Company">
            <img src="/images/banner.jpg" alt="Banner">
        </header>
    </body></html>"#;

    let brand = extract_brand(html, Some("https://example.com"));
    assert_eq!(
        brand.logo_url.as_deref(),
        Some("https://example.com/images/logo.png")
    );
}

#[test]
fn test_logo_by_id() {
    let html = r#"<html><body>
        <header>
            <img id="main-logo" src="/logo.svg" alt="Brand">
        </header>
    </body></html>"#;

    let brand = extract_brand(html, Some("https://example.com"));
    assert_eq!(
        brand.logo_url.as_deref(),
        Some("https://example.com/logo.svg")
    );
}

#[test]
fn test_logo_by_alt() {
    let html = r#"<html><body>
        <header>
            <img src="/brand-logo.png" alt="Acme Corp Logo">
        </header>
    </body></html>"#;

    let brand = extract_brand(html, Some("https://acme.com"));
    assert_eq!(
        brand.logo_url.as_deref(),
        Some("https://acme.com/brand-logo.png")
    );
}

#[test]
fn test_favicon() {
    let html = r#"<html><head>
        <link rel="icon" href="/favicon.ico">
    </head><body></body></html>"#;

    let brand = extract_brand(html, Some("https://example.com"));
    assert_eq!(
        brand.favicon_url.as_deref(),
        Some("https://example.com/favicon.ico")
    );
}

#[test]
fn test_favicon_shortcut_icon() {
    let html = r#"<html><head>
        <link rel="shortcut icon" href="/img/fav.png">
    </head><body></body></html>"#;

    let brand = extract_brand(html, Some("https://example.com"));
    assert_eq!(
        brand.favicon_url.as_deref(),
        Some("https://example.com/img/fav.png")
    );
}

#[test]
fn test_full_brand() {
    let html = r#"
    <html>
    <head>
        <link rel="icon" href="/favicon.ico">
        <style>
            body {
                font-family: "Roboto", "Open Sans", sans-serif;
                background-color: #f5f5f5;
                color: #2d3436;
            }
            .header { background-color: #6c5ce7; }
            .btn-primary { background-color: #6c5ce7; color: #ffeaa7; }
            .btn-secondary { background-color: #00b894; }
            a { color: #0984e3; }
            .border { border-color: #dfe6e9; }
            code { font-family: "JetBrains Mono", monospace; }
        </style>
    </head>
    <body>
        <header class="header">
            <a href="/"><img class="logo" src="/images/logo.svg" alt="Brand"></a>
            <nav>
                <a href="/about">About</a>
            </nav>
        </header>
        <main>
            <h1>Welcome</h1>
            <p>Hello world</p>
        </main>
    </body>
    </html>"#;

    let brand = extract_brand(html, Some("https://example.com"));

    // Colors
    assert!(!brand.colors.is_empty(), "should extract colors");
    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(hexes.contains(&"#6C5CE7"), "should find primary purple");
    assert!(hexes.contains(&"#0984E3"), "should find link blue");
    assert!(hexes.contains(&"#00B894"), "should find secondary green");
    // #f5f5f5 is a boring gray, should be filtered
    assert!(
        !hexes.contains(&"#F5F5F5"),
        "boring gray should be filtered"
    );

    // Fonts
    assert!(brand.fonts.contains(&"Roboto".to_string()));
    assert!(brand.fonts.contains(&"Open Sans".to_string()));
    assert!(brand.fonts.contains(&"JetBrains Mono".to_string()));
    assert!(!brand.fonts.contains(&"sans-serif".to_string()));
    assert!(!brand.fonts.contains(&"monospace".to_string()));

    // Logo
    assert_eq!(
        brand.logo_url.as_deref(),
        Some("https://example.com/images/logo.svg")
    );

    // Favicon
    assert_eq!(
        brand.favicon_url.as_deref(),
        Some("https://example.com/favicon.ico")
    );
}

#[test]
fn test_inline_styles() {
    let html = r#"<html><body>
        <div style="background-color: #e74c3c; color: #ecf0f1;">Content</div>
        <span style="font-family: 'Poppins', sans-serif;">Text</span>
    </body></html>"#;

    let brand = extract_brand(html, None);
    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(hexes.contains(&"#E74C3C"), "should find inline bg color");
    assert!(hexes.contains(&"#ECF0F1"), "should find inline text color");
    assert!(brand.fonts.contains(&"Poppins".to_string()));
}

#[test]
fn test_no_logo_or_favicon() {
    let html = r#"<html><head></head><body><p>Simple page</p></body></html>"#;
    let brand = extract_brand(html, None);
    assert!(brand.logo_url.is_none());
    assert!(brand.favicon_url.is_none());
}

#[test]
fn test_empty_html() {
    let brand = extract_brand("", None);
    assert!(brand.colors.is_empty());
    assert!(brand.fonts.is_empty());
    assert!(brand.logo_url.is_none());
    assert!(brand.favicon_url.is_none());
}

#[test]
fn test_css_custom_properties() {
    let html = r#"<html><head><style>
        :root {
            --primary: #3b82f6;
            --bg-dark: rgb(15, 23, 42);
            --accent: hsl(340, 82%, 52%);
            --spacing: 1rem; /* not a color */
        }
    </style></head><body></body></html>"#;

    let brand = extract_brand(html, None);
    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(hexes.contains(&"#3B82F6"), "should find --primary");
    assert!(hexes.contains(&"#0F172A"), "should find --bg-dark");
    assert!(
        brand.colors.len() >= 3,
        "should find at least 3 colors from vars"
    );
}

#[test]
fn test_tailwind_arbitrary_colors() {
    let html = r#"<html><body>
        <div class="bg-[#1a1a2e] text-[#e94560]">Content</div>
        <span class="border-[rgb(255,107,107)]">Border</span>
    </body></html>"#;

    let brand = extract_brand(html, None);
    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(hexes.contains(&"#1A1A2E"), "should find bg-[#1a1a2e]");
    assert!(hexes.contains(&"#E94560"), "should find text-[#e94560]");
    assert!(
        hexes.contains(&"#FF6B6B"),
        "should find border-[rgb(255,107,107)]"
    );
}

#[test]
fn test_theme_color_meta() {
    let html = r##"<html><head>
        <meta name="theme-color" content="#6366f1">
    </head><body></body></html>"##;

    let brand = extract_brand(html, None);
    let hexes: Vec<&str> = brand.colors.iter().map(|c| c.hex.as_str()).collect();
    assert!(hexes.contains(&"#6366F1"), "should find theme-color");
}

#[test]
fn test_google_fonts_url() {
    let html = r#"<html><head>
        <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;700&family=Roboto+Mono:wght@400">
    </head><body></body></html>"#;

    let brand = extract_brand(html, None);
    assert!(
        brand.fonts.contains(&"Inter".to_string()),
        "should find Inter from Google Fonts URL"
    );
    assert!(
        brand.fonts.contains(&"Roboto Mono".to_string()),
        "should find Roboto Mono from Google Fonts URL"
    );
}

#[test]
fn test_font_preload() {
    let html = r#"<html><head>
        <link rel="preload" as="font" href="/fonts/Geist-Variable.woff2" crossorigin>
        <link rel="preload" as="font" href="/fonts/GeistMono-Regular.woff2" crossorigin>
    </head><body></body></html>"#;

    let brand = extract_brand(html, None);
    assert!(
        brand.fonts.iter().any(|f| f.contains("Geist")),
        "should find Geist from preload"
    );
}

#[test]
fn test_extract_font_name_from_url() {
    assert_eq!(
        extract_font_name_from_url("/fonts/Inter-Variable.woff2"),
        Some("Inter".to_string())
    );
    assert_eq!(
        extract_font_name_from_url("/fonts/Geist-Regular.woff2"),
        Some("Geist".to_string())
    );
    assert_eq!(
        extract_font_name_from_url("/fonts/JetBrainsMono-Bold.woff2"),
        Some("JetBrainsMono".to_string())
    );
}

#[test]
fn test_google_fonts_from_url() {
    let fonts = extract_google_fonts_from_url(
        "https://fonts.googleapis.com/css2?family=Inter:wght@400;700&family=Roboto+Mono:wght@400",
    );
    assert!(fonts.contains(&"Inter".to_string()));
    assert!(fonts.contains(&"Roboto Mono".to_string()));
}

#[test]
fn test_max_10_colors() {
    // Generate HTML with 15 distinct colors
    let colors: Vec<String> = (0..15)
        .map(|i| {
            format!(
                ".c{i} {{ color: #{:02X}{:02X}{:02X}; }}",
                10 + i * 15,
                20 + i * 10,
                30 + i * 5
            )
        })
        .collect();
    let html = format!(
        "<html><head><style>{}</style></head><body></body></html>",
        colors.join("\n")
    );

    let brand = extract_brand(&html, None);
    assert!(brand.colors.len() <= 10, "should cap at 10 colors");
}
