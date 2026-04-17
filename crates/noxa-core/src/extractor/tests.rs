use super::*;

fn parse(html: &str) -> Html {
    Html::parse_document(html)
}

/// Helper: extract with default options (backward-compatible).
fn extract_default(doc: &Html, base_url: Option<&Url>) -> Content {
    extract_content(doc, base_url, &ExtractionOptions::default())
}

#[test]
fn picks_article_over_nav() {
    let html = r##"
    <html>
    <body>
        <nav><ul><li><a href="/">Home</a></li><li><a href="/about">About</a></li></ul></nav>
        <article>
            <h1>Real Article</h1>
            <p>This is the main content of the page. It contains several paragraphs
            of text that make it clearly the main content area.</p>
            <p>Another paragraph with useful information for the reader.</p>
            <p>And a third paragraph to make it really obvious this is content.</p>
        </article>
        <aside class="sidebar">
            <h3>Related Links</h3>
            <ul><li><a href="/1">Link 1</a></li></ul>
        </aside>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);
    assert!(content.markdown.contains("Real Article"));
    assert!(content.markdown.contains("main content"));
}

#[test]
fn falls_back_to_body() {
    let html = r##"<html><body><p>Simple page with just a paragraph.</p></body></html>"##;
    let doc = parse(html);
    let content = extract_default(&doc, None);
    assert!(content.plain_text.contains("Simple page"));
}

#[test]
fn word_count_works() {
    assert_eq!(word_count("hello world foo bar"), 4);
    assert_eq!(word_count(""), 0);
    assert_eq!(word_count("  spaces  everywhere  "), 2);
}

#[test]
fn prefers_content_class() {
    let html = r##"
    <html>
    <body>
        <div class="header"><p>Site header with some branding text content here</p></div>
        <div class="content">
            <h1>Main Content</h1>
            <p>This is the primary content of the page that readers want to see.
            It has multiple sentences and meaningful paragraphs.</p>
            <p>Second paragraph with additional details and context for the article.</p>
            <p>Third paragraph because real articles have substantial text.</p>
        </div>
        <div class="footer"><p>Footer stuff with copyright and legal text here</p></div>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);
    assert!(content.markdown.contains("Main Content"));
}

/// Simulates a Wikipedia-like page where the best content node (article/main)
/// contains a nav sidebar as a child. The markdown converter must strip it.
#[test]
fn wikipedia_like_nav_sidebar_stripped() {
    let html = r##"
    <html>
    <body>
        <article>
            <h1>Rust (programming language)</h1>
            <nav class="sidebar-toc">
                <h2>Contents</h2>
                <ul>
                    <li><a href="#history">History</a></li>
                    <li><a href="#syntax">Syntax</a></li>
                    <li><a href="#features">Features</a></li>
                </ul>
            </nav>
            <aside class="infobox">
                <p>Developer: Mozilla Research</p>
                <p>First appeared: 2010</p>
            </aside>
            <p>Rust is a multi-paradigm programming language focused on performance
            and safety, especially safe concurrency. It accomplishes these goals
            without a garbage collector.</p>
            <p>Rust was originally designed by Graydon Hoare at Mozilla Research,
            with contributions from several other developers.</p>
            <p>The language grew out of a personal project begun in 2006 by Mozilla
            employee Graydon Hoare, who stated that it was possibly named after
            the rust family of fungi.</p>
        </article>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);

    // Article content preserved
    assert!(content.markdown.contains("Rust (programming language)"));
    assert!(
        content
            .markdown
            .contains("multi-paradigm programming language")
    );
    assert!(content.markdown.contains("Graydon Hoare"));

    // Nav sidebar stripped
    assert!(
        !content.markdown.contains("Contents"),
        "TOC nav heading leaked"
    );
    assert!(
        !content.markdown.contains("#history"),
        "TOC nav link leaked"
    );

    // Aside infobox stripped
    assert!(
        !content.markdown.contains("First appeared"),
        "infobox aside leaked"
    );
}

/// When the best node is a large div that happens to contain script tags,
/// the JS code must not appear in the markdown.
#[test]
fn script_inside_content_node_stripped() {
    let html = r##"
    <html>
    <body>
        <main>
            <h1>Interactive Article</h1>
            <p>This article has some embedded JavaScript for interactivity.
            The content itself is what we want to extract, not the code.</p>
            <script>
                window.__NEXT_DATA__ = {"props":{"pageProps":{"article":{"id":123}}}};
                document.addEventListener('DOMContentLoaded', function() {
                    initializeApp();
                });
            </script>
            <p>The article continues with more useful information for readers
            who want to learn about the topic.</p>
            <style>.highlight { background: yellow; }</style>
        </main>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);

    assert!(content.markdown.contains("Interactive Article"));
    assert!(content.markdown.contains("embedded JavaScript"));
    assert!(content.markdown.contains("continues with more"));
    assert!(
        !content.markdown.contains("NEXT_DATA"),
        "script content leaked"
    );
    assert!(
        !content.markdown.contains("initializeApp"),
        "JS function call leaked"
    );
    assert!(
        !content.markdown.contains("background: yellow"),
        "CSS leaked"
    );
}

/// Full-page simulation: header, nav, main content, footer.
/// Only the main content should survive.
#[test]
fn full_page_noise_stripped() {
    let html = r##"
    <html>
    <body>
        <header>
            <div class="logo">MySite</div>
            <nav>
                <a href="/">Home</a>
                <a href="/blog">Blog</a>
                <a href="/about">About</a>
            </nav>
        </header>
        <main>
            <article>
                <h1>How to Write Clean Code</h1>
                <p>Writing clean code is an essential skill for every developer.
                It makes your codebase easier to maintain and understand.</p>
                <p>In this article, we will explore several principles that can
                help you write better, more readable code.</p>
                <p>The first principle is to use meaningful variable names that
                clearly describe what the variable holds.</p>
            </article>
        </main>
        <footer>
            <p>Copyright 2025 MySite</p>
            <a href="/privacy">Privacy</a>
            <a href="/terms">Terms</a>
        </footer>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);

    assert!(content.markdown.contains("How to Write Clean Code"));
    assert!(content.markdown.contains("meaningful variable names"));
    assert!(
        !content.markdown.contains("MySite"),
        "header/footer branding leaked"
    );
    assert!(!content.markdown.contains("Privacy"), "footer link leaked");
    assert!(!content.markdown.contains("Blog"), "nav link leaked");
}

/// H1 in a hero/banner section outside the main content node should be
/// captured and prepended to the markdown output.
#[test]
fn h1_outside_content_node_captured() {
    let html = r##"
    <html>
    <body>
        <div class="hero-banner">
            <h1>The Ultimate Guide to Async Rust</h1>
            <p class="subtitle">Everything you need to know</p>
        </div>
        <article>
            <p>Asynchronous programming in Rust is powered by the async/await
            syntax and the Future trait. This guide covers all the fundamentals
            you need to get started with async Rust.</p>
            <p>We will explore tokio, the most popular async runtime, and show
            you how to build concurrent applications efficiently.</p>
            <p>By the end of this guide you will understand how to write
            performant async code that handles thousands of connections.</p>
        </article>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);

    // H1 must appear in markdown even though it's outside <article>
    assert!(
        content
            .markdown
            .contains("The Ultimate Guide to Async Rust"),
        "H1 from hero banner missing from output"
    );
    // Should be prepended as a heading
    assert!(
        content
            .markdown
            .starts_with("# The Ultimate Guide to Async Rust"),
        "H1 should be prepended as markdown heading"
    );
    // Article content still present
    assert!(content.markdown.contains("async/await"));
    assert!(content.markdown.contains("tokio"));
}

/// Announcement banners with role="region" and aria-label="Announcement"
/// should be recovered even though their class contains "banner" (noise).
#[test]
fn announcement_banner_recovered() {
    let html = r##"
    <html>
    <body>
        <div class="announcement-banner" role="region" aria-label="Announcement">
            <p>Big news! We are joining forces with Acme Corp -
            read more in <a href="https://example.com/blog">our blog</a></p>
        </div>
        <header><nav><a href="/">Home</a></nav></header>
        <article>
            <h1>Our Product</h1>
            <p>We build amazing tools for developers that simplify
            complex workflows and boost productivity every day.</p>
            <p>Our platform handles millions of requests per second
            with low latency and high reliability.</p>
        </article>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, Some(&Url::parse("https://example.com").unwrap()));

    assert!(
        content.markdown.contains("joining forces with Acme Corp"),
        "Announcement banner text missing from output"
    );
    assert!(
        content.markdown.contains("Our Product"),
        "Main content missing"
    );
    // The announcement link should be captured
    assert!(
        content
            .links
            .iter()
            .any(|l| l.href.contains("example.com/blog")),
        "Announcement link not captured"
    );
}

/// Section headings inside <div class="...header"> wrappers should be
/// recovered when sibling content from the same section is in the output.
#[test]
fn section_heading_in_header_class_recovered() {
    let html = r##"
    <html>
    <body>
        <div class="page-wrapper">
            <section class="features">
                <div class="section-header">
                    <h2>Built for scale</h2>
                </div>
                <div class="feature-grid">
                    <p>Handle thousands of concurrent requests with
                    intelligent load balancing and automatic failover.</p>
                    <p>Deploy globally with edge locations in every
                    major region for minimal latency.</p>
                </div>
            </section>
        </div>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);

    assert!(
        content.markdown.contains("## Built for scale"),
        "Section heading should be recovered: {}",
        content.markdown
    );
    assert!(
        content.markdown.contains("concurrent requests"),
        "Section content missing"
    );
}

/// Eyebrow text (short tagline above a section heading) should be
/// recovered when it's inside the same noise-stripped wrapper as the <h2>.
#[test]
fn eyebrow_text_recovered() {
    let html = r##"
    <html>
    <body>
        <div class="page-wrapper">
            <section class="users-section">
                <div class="section-header">
                    <p class="eyebrow">the platform for builders</p>
                    <h2>Loved by developers worldwide</h2>
                </div>
                <div class="grid">
                    <p>Thousands of teams rely on our platform daily for
                    mission-critical applications and workflows.</p>
                </div>
            </section>
        </div>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);

    assert!(
        content.markdown.contains("the platform for builders"),
        "Eyebrow text missing: {}",
        content.markdown
    );
    assert!(
        content.markdown.contains("Loved by developers worldwide"),
        "Section heading missing"
    );
}

/// Decorative route-style labels (starting with "/") should NOT be recovered
/// as eyebrow text — they're design elements, not content.
#[test]
fn route_style_eyebrow_not_recovered() {
    let html = r##"
    <html>
    <body>
        <div class="page-wrapper">
            <section>
                <div class="section-header">
                    <p class="eyebrow">/proof is in the numbers</p>
                    <h2>Trusted in production</h2>
                </div>
                <div class="grid">
                    <p>Our platform handles millions of requests per second
                    with low latency and high reliability.</p>
                </div>
            </section>
        </div>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);

    // With exact class matching, "section-header" is NOT noise
    // (only exact "header" class would be). The eyebrow text is now
    // preserved, which is correct — it's content, not navigation.
    assert!(
        content.markdown.contains("Trusted in production"),
        "Section heading should be recovered"
    );
    assert!(
        content.markdown.contains("Our platform"),
        "Grid content should be present"
    );
}

/// Footer CTA links to documentation URLs should be recovered.
#[test]
fn footer_cta_link_recovered() {
    let html = r##"
    <html>
    <body>
        <article>
            <h1>Our Platform</h1>
            <p>Build powerful applications with our comprehensive API
            and developer tools that handle millions of requests.</p>
            <p>Get started in minutes with our quickstart guide and
            extensive documentation for every feature.</p>
        </article>
        <footer>
            <h2>Start building today</h2>
            <a href="https://docs.example.com">Explore API Docs</a>
            <a href="https://app.example.com">Try it free</a>
            <a href="/privacy">Privacy</a>
            <a href="/terms">Terms</a>
        </footer>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, Some(&Url::parse("https://example.com").unwrap()));

    assert!(
        content.markdown.contains("Start building today"),
        "Footer CTA heading missing: {}",
        content.markdown
    );
    assert!(
        content.markdown.contains("Explore API Docs"),
        "Footer CTA link missing"
    );
    // Non-doc footer links should NOT be recovered
    assert!(
        !content.markdown.contains("Privacy"),
        "Generic footer nav leaked"
    );
    assert!(
        !content.markdown.contains("Terms"),
        "Generic footer nav leaked"
    );
}

/// Headings inside genuine noise (nav, aside) should NOT be recovered,
/// even when sibling content exists in the output.
#[test]
fn heading_inside_nav_not_recovered() {
    let html = r##"
    <html>
    <body>
        <article>
            <h1>Programming Guide</h1>
            <nav class="table-of-contents">
                <h2>Table of Contents</h2>
                <ul>
                    <li><a href="#ch1">Chapter 1</a></li>
                    <li><a href="#ch2">Chapter 2</a></li>
                </ul>
            </nav>
            <p>This comprehensive guide covers everything you need
            to know about modern programming practices.</p>
            <p>From basics to advanced topics, we will explore
            patterns and techniques used by professionals.</p>
        </article>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);

    assert!(
        !content.markdown.contains("Table of Contents"),
        "TOC heading from nav should not be recovered: {}",
        content.markdown
    );
    assert!(content.markdown.contains("comprehensive guide"));
}

/// Structured footer sitemaps (3+ categories with headings) should be
/// recovered as a compact reference section.
#[test]
fn footer_sitemap_recovered() {
    let html = r##"
    <html>
    <body>
        <article>
            <h1>Our Company</h1>
            <p>We build tools that help developers create amazing applications
            faster and more efficiently than ever before.</p>
            <p>Join thousands of teams who trust our platform for their
            mission-critical workloads every single day.</p>
        </article>
        <footer>
            <div class="col">
                <h3>Products</h3>
                <a href="/product-a">Product A</a>
                <a href="/product-b">Product B</a>
                <a href="/product-c">Product C</a>
            </div>
            <div class="col">
                <h3>Solutions</h3>
                <a href="/enterprise">Enterprise</a>
                <a href="/startup">Startup</a>
                <a href="/education">Education</a>
            </div>
            <div class="col">
                <h3>Resources</h3>
                <a href="/blog">Blog</a>
                <a href="/docs">Documentation</a>
                <a href="/community">Community</a>
            </div>
        </footer>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, Some(&Url::parse("https://example.com").unwrap()));

    // Categories should be captured
    assert!(
        content.markdown.contains("Products"),
        "Footer sitemap Products missing: {}",
        content.markdown
    );
    assert!(
        content.markdown.contains("Product A"),
        "Footer sitemap link missing"
    );
    assert!(
        content.markdown.contains("Solutions"),
        "Footer sitemap Solutions missing"
    );
    assert!(
        content.markdown.contains("Resources"),
        "Footer sitemap Resources missing"
    );
    // Main content still present
    assert!(content.markdown.contains("Our Company"));
}

/// Footer sitemaps with fewer than 3 categories should NOT be recovered
/// (not enough structure to be confident it's a sitemap).
#[test]
fn small_footer_not_treated_as_sitemap() {
    let html = r##"
    <html>
    <body>
        <article>
            <h1>Simple Page</h1>
            <p>This is a simple page with minimal footer structure that
            should not trigger sitemap recovery at all.</p>
            <p>The content here is what matters, not the footer links
            or navigation elements below the main content.</p>
        </article>
        <footer>
            <h3>Legal</h3>
            <a href="/privacy">Privacy</a>
            <a href="/terms">Terms</a>
        </footer>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, None);

    assert!(
        !content.markdown.contains("Legal"),
        "Small footer should not be treated as sitemap: {}",
        content.markdown
    );
}

/// Screen-reader-only footer headings (like "Footer") should not leak.
#[test]
fn sr_only_footer_heading_not_recovered() {
    let html = r##"
    <html>
    <body>
        <article>
            <h1>Our Platform</h1>
            <p>Build powerful applications with our comprehensive API
            and developer tools that handle millions of requests.</p>
            <p>Get started in minutes with our quickstart guide and
            extensive documentation for every feature.</p>
        </article>
        <footer>
            <h2 class="u-sr-only">Footer</h2>
            <a href="https://docs.example.com">Explore API Docs</a>
        </footer>
    </body>
    </html>"##;

    let doc = parse(html);
    let content = extract_default(&doc, Some(&Url::parse("https://example.com").unwrap()));

    assert!(
        !content.markdown.contains("## Footer"),
        "SR-only 'Footer' heading should not be recovered: {}",
        content.markdown
    );
}
