use super::*;

// -- HTML entity decoding --

#[test]
fn decode_nbsp() {
    assert_eq!(
        decode_html_entities("From overview to deep details,&nbsp;fast"),
        "From overview to deep details, fast"
    );
}

#[test]
fn decode_amp_lt_gt() {
    assert_eq!(decode_html_entities("A &amp; B"), "A & B");
    assert_eq!(decode_html_entities("&lt;div&gt;"), "<div>");
}

#[test]
fn decode_numeric_entities() {
    assert_eq!(decode_html_entities("&#169;"), "\u{00A9}"); // (C)
    assert_eq!(decode_html_entities("&#x2019;"), "\u{2019}"); // '
}

#[test]
fn decode_no_entity_passthrough() {
    let input = "Normal text without entities";
    assert_eq!(decode_html_entities(input), input);
}

#[test]
fn decode_named_entities() {
    assert_eq!(decode_html_entities("&mdash;"), "\u{2014}");
    assert_eq!(decode_html_entities("&copy;"), "\u{00A9}");
    assert_eq!(decode_html_entities("&hellip;"), "\u{2026}");
}

#[test]
fn decode_unknown_entity_preserved() {
    assert_eq!(decode_html_entities("&foobar;"), "&foobar;");
}

// -- UI control text filtering --

#[test]
fn ui_control_material_icons() {
    assert!(is_ui_control_line("navigate_before navigate_next"));
    assert!(is_ui_control_line("chevron_left"));
    assert!(is_ui_control_line("arrow_back arrow_forward"));
    assert!(is_ui_control_line("expand_more"));
}

#[test]
fn ui_control_arrow_chars() {
    assert!(is_ui_control_line("\u{2190} \u{2192}"));
    assert!(is_ui_control_line("\u{203A}"));
}

#[test]
fn ui_control_not_real_content() {
    assert!(!is_ui_control_line("Navigate to the next page"));
    assert!(!is_ui_control_line("Click the menu button to expand"));
    assert!(!is_ui_control_line("Search for products"));
    assert!(!is_ui_control_line(""));
}

#[test]
fn ui_control_strip_from_text() {
    let input = "Hello\nnavigate_before navigate_next\nWorld";
    assert_eq!(strip_ui_control_text(input), "Hello\nWorld");
}

// -- Long alt-text descriptions --

#[test]
fn long_alt_description_detected() {
    assert!(is_long_alt_description(
        "An illustration in the style of lo-fi anime showing a cute dinosaur coding on a laptop in a cozy room with lots of details."
    ));
    assert!(is_long_alt_description(
        "A screenshot showing the dashboard interface with multiple panels and configuration options for the user."
    ));
    assert!(is_long_alt_description(
        "This element contains an interactive demo for sighted users. It's a demonstration of Cursor's IDE showing AI-powered features."
    ));
}

#[test]
fn long_alt_description_real_content_kept() {
    assert!(!is_long_alt_description("An illustration")); // too short
    assert!(!is_long_alt_description(
        "The quick brown fox jumps over the lazy dog and keeps running for a very long time across the field."
    ));
    assert!(!is_long_alt_description(
        "# An illustration of the main heading which is quite long and spans multiple words for testing purposes."
    ));
}

// -- CSS artifact filtering --

#[test]
fn css_artifact_keyframes_stripped() {
    let input = "curl -fsSL https://deno.land/install.sh | sh@keyframes copy{from{background:var(--runtime)}";
    let out = strip_css_artifacts(input);
    assert_eq!(out, "curl -fsSL https://deno.land/install.sh | sh");
}

#[test]
fn css_artifact_standalone_line() {
    assert!(is_css_artifact_line("selector{property:value}"));
}

#[test]
fn css_artifact_not_real_code() {
    assert!(!is_css_artifact_line("let x = { key: value };"));
    assert!(!is_css_artifact_line("# Heading with {braces}"));
    assert!(!is_css_artifact_line("@username mentioned you"));
}

// -- Leaked JavaScript stripping --

#[test]
fn leaked_js_self_wrap() {
    let input = "## Accelerate speed, reduce riskself.__wrap_n=self.__wrap_n||(self.CSS&&CSS.supports(\"text-wrap\",\"balance\")?1:2);";
    let result = strip_leaked_js(input);
    assert_eq!(result, "## Accelerate speed, reduce risk");
}

#[test]
fn leaked_js_normal_text_preserved() {
    let input = "Normal text without any JavaScript";
    assert_eq!(strip_leaked_js(input), input);
}

#[test]
fn leaked_js_code_block_preserved() {
    let input = "```\nself.__wrap_n = 42;\n```";
    assert_eq!(strip_leaked_js(input), input);
}

// -- Invisible Unicode stripping --

#[test]
fn invisible_unicode_stripped() {
    let input = "Hello\u{200B}World\u{200D}Test\u{FEFF}End";
    assert_eq!(strip_invisible_unicode(input), "HelloWorldTestEnd");
}

#[test]
fn invisible_unicode_no_change() {
    let input = "Normal visible text";
    assert_eq!(strip_invisible_unicode(input), input);
}

// -- Collapse spaced text --

#[test]
fn collapse_spaced_text_basic() {
    assert_eq!(
        collapse_spaced_text("S t a r t D e p l o y i n g"),
        "Start Deploying"
    );
}

#[test]
fn collapse_spaced_text_single_word() {
    assert_eq!(collapse_spaced_text("H e l l o"), "Hello");
}

#[test]
fn collapse_spaced_text_skips_code_blocks() {
    let input = "```\nS t a r t\n```";
    assert_eq!(collapse_spaced_text(input), input);
}

#[test]
fn collapse_spaced_text_short_ignored() {
    // Only 3 real chars -- below threshold of 4
    assert_eq!(collapse_spaced_text("a b c"), "a b c");
}

#[test]
fn collapse_spaced_text_mixed_line() {
    assert_eq!(
        collapse_spaced_text("Welcome to S t a r t"),
        "Welcome to Start"
    );
}

// -- Word list collapsing --

#[test]
fn long_api_list_collapsed() {
    let words: Vec<&str> = vec![
        "Worker",
        "MessageEvent",
        "WritableStreamDefaultController",
        "DecompressionStream",
        "CompressionStream",
        "Blob",
        "Response",
        "EventTarget",
        "WebSocket",
        "CryptoKey",
        "ErrorEvent",
        "PerformanceMark",
        "WorkerNavigator",
        "TextDecoder",
        "TextEncoder",
        "TransformStream",
        "File",
        "CustomEvent",
        "Event",
        "DOMException",
        "ReadableStream",
        "Storage",
        "WebAssembly",
        "URLSearchParams",
        "ProgressEvent",
        "FileReader",
    ];
    let line = words.join(" ");
    let input = format!("Some prefix text {line}");
    let result = collapse_word_lists(&input);
    assert!(result.contains("... and"), "should collapse: {result}");
}

#[test]
fn normal_prose_not_collapsed() {
    let input = "This is a perfectly normal paragraph with lots of words but they are all lowercase prose that should not be collapsed because it's actual content.";
    assert_eq!(collapse_word_lists(input), input);
}

// -- Adjacent description dedup --

#[test]
fn adjacent_description_deduped() {
    let input = "Infrastructure From overview to deep details, fast\nLearn more ** From overview to deep details, fast\nAPM Monitor performance";
    let result = dedup_adjacent_descriptions(input);
    assert!(result.contains("Infrastructure From overview"));
    assert!(!result.contains("Learn more ** From overview"));
    assert!(result.contains("APM Monitor"));
}

#[test]
fn non_duplicate_learn_more_preserved() {
    let input = "Product A does something\nLearn more about different things\nProduct B";
    assert_eq!(dedup_adjacent_descriptions(input), input);
}

// -- Alt text noise --

#[test]
fn alt_text_descriptive_prefix_stripped() {
    let input = "Hello\nImage of Glossier website selling beauty products\nWorld";
    assert_eq!(strip_alt_text_noise(input), "Hello\nWorld");
}

#[test]
fn alt_text_photo_prefix_stripped() {
    let input = "Text\nPhoto of customer at plant retailer The Sill\nMore";
    assert_eq!(strip_alt_text_noise(input), "Text\nMore");
}

#[test]
fn alt_text_animation_stripped() {
    let input = "Above\nAnimation of example abandoned cart email with graph\nBelow";
    assert_eq!(strip_alt_text_noise(input), "Above\nBelow");
}

#[test]
fn alt_text_normal_prose_kept() {
    let input = "Image quality is important for this use case";
    assert_eq!(strip_alt_text_noise(input), input);
}

#[test]
fn alt_text_short_prefix_kept() {
    let input = "Image of X";
    assert_eq!(strip_alt_text_noise(input), input);
}

#[test]
fn broken_image_fragment_stripped() {
    assert_eq!(strip_alt_text_noise("Hello\n.webp)\nWorld"), "Hello\nWorld");
    assert_eq!(
        strip_alt_text_noise("Text\n.svg)                .webp)\nMore"),
        "Text\nMore"
    );
}

#[test]
fn social_avatar_labels_stripped() {
    let input = "@a twitter image, @b twitter image, @c twitter image";
    assert_eq!(strip_alt_text_noise(input), "");
}

#[test]
fn repeated_brand_list_stripped() {
    let input =
        "Supabase DB, Supabase Auth, Supabase Functions, Supabase Storage, Supabase Vector";
    assert_eq!(strip_alt_text_noise(input), "");
}

#[test]
fn alt_text_code_block_preserved() {
    let input = "```\nImage of something in code\n```";
    assert_eq!(strip_alt_text_noise(input), input);
}
