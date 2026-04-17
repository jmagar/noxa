use super::*;

#[test]
fn aspnet_form_content_extraction() {
    let content = "x".repeat(600); // Ensure >500 chars
    let html = format!(
        r#"<html><body>
        <form method="post" action="./page.aspx" id="form1">
            <div class="wrapper">
                <div class="header"><a href="/">Logo</a></div>
                <div class="content">
                    <h2>Section</h2>
                    <h3>Question?</h3>
                    <p>{content}</p>
                </div>
            </div>
        </form>
    </body></html>"#
    );
    let doc = Html::parse_document(&html);
    let opts = ExtractionOptions::default();
    let result = extract_content(&doc, None, &opts);
    assert!(
        result.markdown.contains("Section"),
        "h2 missing from markdown"
    );
    assert!(
        result.markdown.contains("Question"),
        "h3 missing from markdown"
    );
}

/// Simulate unclosed header div absorbing the content div.
/// The header's noise class should NOT propagate to the absorbed content
/// because the safety valve detects the header has >5000 chars (broken wrapper).
#[test]
fn unclosed_header_div_does_not_swallow_content() {
    let faq = "Lorem ipsum dolor sit amet. ".repeat(300); // ~8400 chars
    // The header div is intentionally NOT closed — the HTML parser makes
    // div.content a child of div.header. The safety valve (>5000 chars)
    // should prevent div.header from being treated as noise.
    let html = format!(
        r#"<html><body>
        <div class="wrapper">
            <div class="header"><a href="/">Logo</a>
            <div class="content">
                <h2>FAQ Section</h2>
                <h3>First question?</h3>
                <p>{faq}</p>
            </div>
        </div>
    </body></html>"#
    );
    let doc = Html::parse_document(&html);
    let opts = ExtractionOptions::default();
    let result = extract_content(&doc, None, &opts);
    assert!(
        result.markdown.contains("FAQ Section"),
        "h2 missing: header swallowed content"
    );
    assert!(
        result.markdown.contains("First question"),
        "h3 missing: header swallowed content"
    );
}
