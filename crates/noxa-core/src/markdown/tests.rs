use super::*;
use scraper::Html;

fn convert_html(html: &str, base: Option<&str>) -> (String, String, ConvertedAssets) {
    let doc = Html::parse_fragment(html);
    let root = doc.root_element();
    let base_url = base.and_then(|u| Url::parse(u).ok());
    convert(root, base_url.as_ref(), &HashSet::new())
}

#[test]
fn headings() {
    let (md, _, _) = convert_html("<h1>Title</h1>", None);
    assert!(md.contains("# Title"));

    let (md, _, _) = convert_html("<h3>Sub</h3>", None);
    assert!(md.contains("### Sub"));
}

#[test]
fn paragraphs_and_inline() {
    let (md, _, _) = convert_html(
        "<p>Hello <strong>world</strong> and <em>stuff</em></p>",
        None,
    );
    assert!(md.contains("Hello **world** and *stuff*"));
}

#[test]
fn links_collected() {
    let (md, _, assets) = convert_html(
        r#"<p><a href="https://example.com">Click here</a></p>"#,
        None,
    );
    assert!(md.contains("[Click here](https://example.com)"));
    assert_eq!(assets.links.len(), 1);
    assert_eq!(assets.links[0].href, "https://example.com");
}

#[test]
fn relative_url_resolution() {
    let (md, _, _) = convert_html(
        r#"<a href="/about">About</a>"#,
        Some("https://example.com/page"),
    );
    assert!(md.contains("[About](https://example.com/about)"));
}

#[test]
fn images_collected() {
    let (md, _, assets) = convert_html(
        r#"<img src="https://img.example.com/photo.jpg" alt="A photo">"#,
        None,
    );
    assert!(md.contains("![A photo](https://img.example.com/photo.jpg)"));
    assert_eq!(assets.images.len(), 1);
}

#[test]
fn code_blocks() {
    let (md, _, assets) = convert_html(
        r#"<pre><code class="language-rust">fn main() {}</code></pre>"#,
        None,
    );
    assert!(md.contains("```rust"));
    assert!(md.contains("fn main() {}"));
    assert_eq!(assets.code_blocks.len(), 1);
    assert_eq!(assets.code_blocks[0].language.as_deref(), Some("rust"));
}

#[test]
fn multiline_code_preserves_newlines() {
    let html = "<pre><code class=\"language-js\">function App() {\n  const [count, setCount] = useState(0);\n  return count;\n}</code></pre>";
    let (md, _, assets) = convert_html(html, None);
    assert!(md.contains("```js"), "missing language fence: {md}");
    assert!(
        md.contains("function App() {\n  const [count, setCount] = useState(0);"),
        "newlines collapsed in code block: {md}"
    );
    assert_eq!(assets.code_blocks.len(), 1);
    assert_eq!(assets.code_blocks[0].language.as_deref(), Some("js"));
}

#[test]
fn multiline_code_with_br_tags() {
    let html = "<pre><code class=\"language-js\">function App() {<br>  const x = 1;<br>  return x;<br>}</code></pre>";
    let (md, _, _) = convert_html(html, None);
    assert!(md.contains("```js"), "missing language fence: {md}");
    assert!(
        md.contains("function App() {\n  const x = 1;\n  return x;\n}"),
        "br tags not converted to newlines in code block: {md}"
    );
}

#[test]
fn multiline_code_with_div_lines() {
    let html = "<pre><code class=\"language-py\"><div>def hello():</div><div>    print(\"hi\")</div></code></pre>";
    let (md, _, _) = convert_html(html, None);
    assert!(md.contains("```py"), "missing language fence: {md}");
    assert!(
        md.contains("def hello():\n"),
        "div-separated lines not preserved in code block: {md}"
    );
}

#[test]
fn multiline_code_with_span_children() {
    let html = "<pre><code class=\"language-js\"><span class=\"token keyword\">function</span> <span class=\"token function\">App</span>() {\n  <span class=\"token keyword\">const</span> [count, setCount] = useState(0);\n  <span class=\"token keyword\">return</span> count;\n}</code></pre>";
    let (md, _, assets) = convert_html(html, None);
    assert!(md.contains("```js"), "missing language fence: {md}");
    assert!(
        md.contains("function App() {\n  const"),
        "newlines collapsed in highlighted code block: {md}"
    );
    assert_eq!(assets.code_blocks.len(), 1);
}

#[test]
fn multiline_code_no_inline_markdown() {
    let html = "<pre><code>let **x** = *y*;\nlet a = b;</code></pre>";
    let (md, _, _) = convert_html(html, None);
    assert!(
        md.contains("let **x** = *y*;"),
        "code block content was processed for inline markdown: {md}"
    );
}

#[test]
fn inline_code() {
    let (md, _, _) = convert_html("<p>Use <code>cargo build</code> to compile</p>", None);
    assert!(md.contains("`cargo build`"));
}

#[test]
fn unordered_list() {
    let (md, _, _) = convert_html("<ul><li>Alpha</li><li>Beta</li></ul>", None);
    assert!(md.contains("- Alpha"));
    assert!(md.contains("- Beta"));
}

#[test]
fn ordered_list() {
    let (md, _, _) = convert_html("<ol><li>First</li><li>Second</li></ol>", None);
    assert!(md.contains("1. First"));
    assert!(md.contains("2. Second"));
}

#[test]
fn blockquote() {
    let (md, _, _) = convert_html("<blockquote><p>A wise quote</p></blockquote>", None);
    assert!(md.contains("> A wise quote"));
}

#[test]
fn table() {
    let html = r##"
    <table>
        <thead><tr><th>Name</th><th>Age</th></tr></thead>
        <tbody><tr><td>Alice</td><td>30</td></tr></tbody>
    </table>"##;
    let (md, _, _) = convert_html(html, None);
    assert!(md.contains("| Name | Age |"));
    assert!(md.contains("| --- | --- |"));
    assert!(md.contains("| Alice | 30 |"));
}

#[test]
fn layout_table() {
    // Layout tables (cells with block elements) should render as sections, not markdown tables
    let html = r##"
    <table>
        <tr>
            <td>
                <p>Column one first paragraph</p>
                <p>Column one second paragraph</p>
            </td>
            <td>
                <p>Column two content</p>
                <hr>
                <p>Column two after rule</p>
            </td>
        </tr>
    </table>"##;
    let (md, _, _) = convert_html(html, None);
    // Should NOT produce markdown table syntax
    assert!(
        !md.contains("| "),
        "layout table should not use pipe syntax: {md}"
    );
    // Should contain the content as separate blocks
    assert!(
        md.contains("Column one first paragraph"),
        "missing content: {md}"
    );
    assert!(md.contains("Column two content"), "missing content: {md}");
    assert!(
        md.contains("Column two after rule"),
        "missing content: {md}"
    );
}

#[test]
fn layout_table_with_links() {
    // Drudge-style layout: cells full of links and divs
    let html = r##"
    <table>
        <tr>
            <td>
                <div><a href="https://example.com/1">Headline One</a></div>
                <div><a href="https://example.com/2">Headline Two</a></div>
            </td>
            <td>
                <div><a href="https://example.com/3">Headline Three</a></div>
            </td>
        </tr>
    </table>"##;
    let (md, _, _) = convert_html(html, None);
    assert!(
        !md.contains("| "),
        "layout table should not use pipe syntax: {md}"
    );
    assert!(
        md.contains("[Headline One](https://example.com/1)"),
        "missing link: {md}"
    );
    assert!(
        md.contains("[Headline Two](https://example.com/2)"),
        "missing link: {md}"
    );
    assert!(
        md.contains("[Headline Three](https://example.com/3)"),
        "missing link: {md}"
    );
}

#[test]
fn horizontal_rule() {
    let (md, _, _) = convert_html("<p>Above</p><hr><p>Below</p>", None);
    assert!(md.contains("---"));
}

#[test]
fn strips_to_plain_text() {
    let (_, plain, _) = convert_html(
        "<p>Hello <strong>bold</strong> <a href='#'>link</a></p>",
        None,
    );
    assert!(plain.contains("Hello bold link"));
    assert!(!plain.contains("**"));
    assert!(!plain.contains("["));
}

#[test]
fn strips_table_syntax_from_plain_text() {
    let html = r##"
    <table>
        <thead><tr><th>Name</th><th>Age</th></tr></thead>
        <tbody><tr><td>Alice</td><td>30</td></tr></tbody>
    </table>"##;
    let (md, plain, _) = convert_html(html, None);
    // Markdown should have table syntax
    assert!(md.contains("| --- |"));
    // Plain text should NOT have any pipe or separator syntax
    assert!(!plain.contains("| --- |"), "separator row leaked: {plain}");
    assert!(!plain.contains("| Name"), "pipe syntax leaked: {plain}");
    assert!(plain.contains("Name"), "table content missing: {plain}");
    assert!(plain.contains("Alice"), "table content missing: {plain}");
}

#[test]
fn nested_list() {
    let html = r##"
    <ul>
        <li>Top
            <ul>
                <li>Nested</li>
            </ul>
        </li>
    </ul>"##;
    let (md, _, _) = convert_html(html, None);
    assert!(md.contains("- Top"));
    assert!(md.contains("  - Nested"));
}

// --- Noise stripping tests ---

#[test]
fn strips_nav_sidebar_from_content() {
    let html = r##"
    <div>
        <nav>
            <ul>
                <li><a href="/">Home</a></li>
                <li><a href="/about">About</a></li>
                <li><a href="/contact">Contact</a></li>
            </ul>
        </nav>
        <div class="sidebar">
            <h3>Related Articles</h3>
            <ul><li><a href="/other">Other article</a></li></ul>
        </div>
        <article>
            <h1>Main Article Title</h1>
            <p>This is the actual content that readers care about.</p>
        </article>
    </div>"##;
    let (md, plain, _) = convert_html(html, None);

    assert!(md.contains("Main Article Title"));
    assert!(md.contains("actual content"));
    assert!(!md.contains("Home"), "nav link 'Home' leaked into output");
    assert!(!md.contains("About"), "nav link 'About' leaked into output");
    assert!(
        !md.contains("Related Articles"),
        "sidebar heading leaked into output"
    );
    assert!(
        !plain.contains("Other article"),
        "sidebar link leaked into plain text"
    );
}

#[test]
fn strips_script_content() {
    let html = r##"
    <div>
        <p>Real content here.</p>
        <script>
            var React = require('react');
            window.__NEXT_DATA__ = {"props":{"pageProps":{}}};
            console.log("hydration complete");
        </script>
        <script type="application/json">{"key": "value"}</script>
        <p>More real content.</p>
    </div>"##;
    let (md, _, _) = convert_html(html, None);

    assert!(md.contains("Real content here"));
    assert!(md.contains("More real content"));
    assert!(!md.contains("React"), "script variable leaked into output");
    assert!(
        !md.contains("NEXT_DATA"),
        "React hydration data leaked into output"
    );
    assert!(!md.contains("console.log"), "JS code leaked into output");
    assert!(
        !md.contains(r#""key""#),
        "JSON script content leaked into output"
    );
}

#[test]
fn strips_style_content() {
    let html = r##"
    <div>
        <style>
            .article { font-size: 16px; color: #333; }
            body { margin: 0; }
        </style>
        <p>Styled paragraph content.</p>
    </div>"##;
    let (md, _, _) = convert_html(html, None);

    assert!(md.contains("Styled paragraph content"));
    assert!(!md.contains("font-size"), "CSS leaked into output");
    assert!(!md.contains("margin"), "CSS leaked into output");
}

#[test]
fn strips_footer_content() {
    let html = r##"
    <div>
        <p>Article body text with important information.</p>
        <footer>
            <p>Copyright 2025 Example Corp. All rights reserved.</p>
            <nav>
                <a href="/privacy">Privacy Policy</a>
                <a href="/terms">Terms of Service</a>
            </nav>
        </footer>
    </div>"##;
    let (md, _, _) = convert_html(html, None);

    assert!(md.contains("Article body text"));
    assert!(!md.contains("Copyright"), "footer text leaked into output");
    assert!(
        !md.contains("Privacy Policy"),
        "footer nav leaked into output"
    );
}

#[test]
fn strips_by_role_attribute() {
    let html = r##"
    <div>
        <div role="navigation"><a href="/">Home</a><a href="/docs">Docs</a></div>
        <div role="banner"><h1>Site Banner</h1></div>
        <div role="main">
            <p>The main content lives here.</p>
        </div>
        <div role="complementary"><p>Sidebar widget</p></div>
        <div role="contentinfo"><p>Footer info</p></div>
    </div>"##;
    let (md, _, _) = convert_html(html, None);

    assert!(md.contains("main content lives here"));
    assert!(!md.contains("Site Banner"), "banner role leaked");
    assert!(!md.contains("Sidebar widget"), "complementary role leaked");
    assert!(!md.contains("Footer info"), "contentinfo role leaked");
    assert!(!md.contains("Docs"), "navigation role leaked");
}

#[test]
fn strips_by_class_patterns() {
    // Uses exact class token matching.
    // "cookie" matches class="cookie", not class="cookie-banner".
    let html = r##"
    <div>
        <div class="cookie"><p>We use cookies</p></div>
        <div class="social"><a href="#">Share on Twitter</a></div>
        <div class="sidebar"><p>Sidebar content here</p></div>
        <div class="modal"><p>Subscribe to newsletter</p></div>
        <p>This is the real article content.</p>
    </div>"##;
    let (md, _, _) = convert_html(html, None);

    assert!(md.contains("real article content"));
    assert!(!md.contains("cookies"), "cookie class leaked");
    assert!(!md.contains("Twitter"), "social class leaked");
    assert!(!md.contains("Sidebar content"), "sidebar class leaked");
    assert!(!md.contains("Subscribe"), "modal class leaked");
}

#[test]
fn compound_classes_not_noise() {
    // Compound class names should NOT trigger noise filter.
    // "free-modal-container" is Vice.com's content wrapper, not a modal.
    let html = r##"
    <div>
        <div class="free-modal-container"><p>Vice article content here</p></div>
        <div class="social-share"><a href="#">Share link</a></div>
        <div class="cookie-banner"><p>Cookie notice</p></div>
        <p>Main content.</p>
    </div>"##;
    let (md, _, _) = convert_html(html, None);

    assert!(
        md.contains("Vice article content"),
        "compound modal class should not be noise"
    );
    assert!(
        md.contains("Share link"),
        "social-share should not be noise"
    );
    assert!(
        md.contains("Cookie notice"),
        "cookie-banner should not be noise"
    );
}

#[test]
fn strips_by_id_patterns() {
    // Exact ID matching — "sidebar" matches, "sidebar-left" does NOT.
    let html = r##"
    <div>
        <div id="sidebar"><p>Sidebar content</p></div>
        <div id="nav"><a href="/">Home</a></div>
        <div id="cookie"><p>Accept cookies?</p></div>
        <p>Article text that matters.</p>
    </div>"##;
    let (md, _, _) = convert_html(html, None);

    assert!(md.contains("Article text that matters"));
    assert!(!md.contains("Sidebar content"), "sidebar id leaked");
    assert!(!md.contains("Accept cookies"), "cookie id leaked");
}

#[test]
fn preserves_content_with_no_noise() {
    let html = r##"
    <div>
        <h1>Clean Article</h1>
        <p>First paragraph with <strong>bold</strong> and <em>italic</em>.</p>
        <p>Second paragraph with a <a href="https://example.com">link</a>.</p>
        <pre><code class="language-python">print("hello")</code></pre>
        <blockquote><p>A great quote.</p></blockquote>
    </div>"##;
    let (md, _, assets) = convert_html(html, None);

    assert!(md.contains("# Clean Article"));
    assert!(md.contains("**bold**"));
    assert!(md.contains("*italic*"));
    assert!(md.contains("[link](https://example.com)"));
    assert!(md.contains("```python"));
    assert!(md.contains("> A great quote."));
    assert_eq!(assets.links.len(), 1);
    assert_eq!(assets.code_blocks.len(), 1);
}

#[test]
fn ad_class_does_not_false_positive() {
    // "ad" as substring in "read", "loading", "load" should NOT be stripped
    let html = r##"
    <div>
        <div class="reading-time"><p>5 min read</p></div>
        <div class="loading-indicator"><p>Loading content</p></div>
        <p>Main text.</p>
    </div>"##;
    let (md, _, _) = convert_html(html, None);

    assert!(
        md.contains("5 min read"),
        "reading-time was incorrectly stripped"
    );
    assert!(
        md.contains("Loading content"),
        "loading-indicator was incorrectly stripped"
    );
    assert!(md.contains("Main text"));
}

// --- Adjacent inline element spacing tests ---

#[test]
fn adjacent_buttons_get_separated() {
    let html =
        r#"<div><button>search</button><button>extract</button><button>crawl</button></div>"#;
    let (md, _, _) = convert_html(html, None);
    assert!(
        !md.contains("searchextract"),
        "adjacent buttons mashed: {md}"
    );
    assert!(
        !md.contains("extractcrawl"),
        "adjacent buttons mashed: {md}"
    );
}

#[test]
fn adjacent_links_get_separated() {
    let html = r#"<div><a href="/a">Talk to an expert</a><a href="/b">Try it out</a></div>"#;
    let (md, _, _) = convert_html(html, None);
    assert!(
        !md.contains("expert)["),
        "adjacent links should have space: {md}"
    );
}

#[test]
fn adjacent_spans_get_separated() {
    let html = r#"<div><span>Hello</span><span>World</span></div>"#;
    let (md, _, _) = convert_html(html, None);
    assert!(!md.contains("HelloWorld"), "adjacent spans mashed: {md}");
}

#[test]
fn inline_text_with_adjacent_elements() {
    // Inside a <p>, adjacent inline elements should also be separated
    let html = r#"<p><a href="/a">One</a><a href="/b">Two</a><a href="/c">Three</a></p>"#;
    let (md, _, _) = convert_html(html, None);
    assert!(
        !md.contains(")("),
        "adjacent links in paragraph mashed: {md}"
    );
}

#[test]
fn no_extra_space_when_whitespace_exists() {
    // When HTML already has whitespace, don't double-space
    let html = r#"<p><a href="/a">One</a> <a href="/b">Two</a></p>"#;
    let (md, _, _) = convert_html(html, None);
    assert!(!md.contains("  "), "double space introduced: {md}");
}

// --- Code block indentation tests ---
// Syntax highlighters (Prism.js, Shiki, highlight.js) wrap tokens in <span>
// elements. Leading whitespace (indentation) appears as text nodes between
// these spans. collect_preformatted_text must preserve all whitespace verbatim,
// and collapse_whitespace must not strip leading spaces inside fenced code blocks.

#[test]
fn syntax_highlighted_code_preserves_indentation() {
    // Mimics React docs Prism.js output where each token is a <span>
    // and indentation is a text node between closing/opening spans.
    let html = r#"<pre><code class="language-js"><span class="token keyword">function</span> <span class="token function">MyComponent</span><span class="token punctuation">(</span><span class="token punctuation">)</span> <span class="token punctuation">{</span>
  <span class="token keyword">const</span> <span class="token punctuation">[</span>age<span class="token punctuation">,</span> setAge<span class="token punctuation">]</span> <span class="token operator">=</span> <span class="token function">useState</span><span class="token punctuation">(</span><span class="token number">28</span><span class="token punctuation">)</span><span class="token punctuation">;</span>
<span class="token punctuation">}</span></code></pre>"#;

    let (md, _, assets) = convert_html(html, None);

    assert!(md.contains("```js"), "missing language fence: {md}");
    assert!(
        md.contains("function MyComponent() {"),
        "first line wrong: {md}"
    );
    assert!(
        md.contains("  const [age, setAge] = useState(28);"),
        "indentation not preserved in syntax-highlighted code: {md}"
    );
    assert!(md.contains("\n}"), "closing brace missing: {md}");
    assert_eq!(assets.code_blocks.len(), 1);
    assert_eq!(assets.code_blocks[0].language.as_deref(), Some("js"));
}

#[test]
fn shiki_line_spans_preserve_indentation() {
    // Shiki wraps each line in <span class="line">, indentation is a text
    // node inside the line span.
    let html = concat!(
        r#"<pre><code class="language-js">"#,
        r#"<span class="line"><span class="token keyword">function</span> foo() {</span>"#,
        "\n",
        r#"<span class="line">  <span class="token keyword">return</span> 1;</span>"#,
        "\n",
        r#"<span class="line">}</span>"#,
        r#"</code></pre>"#,
    );
    let (md, _, _) = convert_html(html, None);
    assert!(
        md.contains("  return 1;"),
        "Shiki-style indentation lost: {md}"
    );
}

#[test]
fn deep_indentation_preserved_in_code() {
    // Multiple nesting levels -- 4-space indentation
    let html = concat!(
        "<pre><code class=\"language-py\">",
        "def outer():\n",
        "    def inner():\n",
        "        return 42\n",
        "    return inner",
        "</code></pre>"
    );
    let (md, _, _) = convert_html(html, None);
    assert!(md.contains("    def inner():"), "4-space indent lost: {md}");
    assert!(
        md.contains("        return 42"),
        "8-space indent lost: {md}"
    );
}

#[test]
fn tab_indentation_preserved_in_code() {
    let html = "<pre><code>if (x) {\n\treturn;\n}</code></pre>";
    let (md, _, _) = convert_html(html, None);
    assert!(md.contains("\treturn;"), "tab indentation lost: {md}");
}

#[test]
fn collapse_whitespace_skips_code_fences() {
    // Directly test that collapse_whitespace bypasses code block content
    let input = "text\n\n```js\nfunction foo() {\n  const x = 1;\n    if (true) {\n      return;\n    }\n}\n```\n\nmore text";
    let output = collapse_whitespace(input);
    assert!(
        output.contains("  const x = 1;"),
        "collapse_whitespace stripped 2-space indent: {output}"
    );
    assert!(
        output.contains("    if (true) {"),
        "collapse_whitespace stripped 4-space indent: {output}"
    );
    assert!(
        output.contains("      return;"),
        "collapse_whitespace stripped 6-space indent: {output}"
    );
}
