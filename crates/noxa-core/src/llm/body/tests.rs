use super::*;

#[test]
fn collapse_repeated_phrase_in_line() {
    let input = "talk play chat hang out talk play chat hang out";
    let result = collapse_repeated_in_line(input);
    assert_eq!(result, "talk play chat hang out");
}

#[test]
fn collapse_repeated_phrase_triple() {
    let input = "go home go home go home";
    let result = collapse_repeated_in_line(input);
    assert_eq!(result, "go home");
}

// -- heading dedup --

#[test]
fn dedup_duplicate_headings_removes() {
    let input =
        "## Features\n\nGreat stuff\n\n## Other\n\nMore\n\n## Features\n\nGreat stuff\n";
    let result = dedup_duplicate_headings(input);
    assert_eq!(result.matches("## Features").count(), 1);
    assert!(result.starts_with("## Features"));
}

#[test]
fn dedup_duplicate_headings_different_levels() {
    let input = "## Foo\n\nContent\n\n### Foo\n\nOther\n";
    let result = dedup_duplicate_headings(input);
    assert!(result.contains("## Foo"));
    assert!(result.contains("### Foo"));
}

#[test]
fn dedup_duplicate_headings_no_dupes() {
    let input = "## A\n\nText\n\n## B\n\nMore\n";
    assert_eq!(dedup_duplicate_headings(input), input);
}

#[test]
fn dedup_duplicate_headings_removes_following_content() {
    let input =
        "## Setup\n\nStep 1\nStep 2\n\n## Other\n\nStuff\n\n## Setup\n\nStep 1\nStep 2\n";
    let result = dedup_duplicate_headings(input);
    assert_eq!(result.matches("## Setup").count(), 1);
    assert_eq!(result.matches("Step 1").count(), 1);
    assert_eq!(result.matches("Step 2").count(), 1);
}

// -- comma list dedup --

#[test]
fn dedup_comma_list_catches_repeated_logos() {
    let input = "mozilla, github, 1password, pwc, mozilla, github, 1password, pwc, mozilla, github, 1password, pwc";
    let out = dedup_comma_lists(input);
    assert_eq!(out, "mozilla, github, 1password, pwc");
}

#[test]
fn dedup_comma_list_preserves_unique_list() {
    let input = "apple, banana, cherry, date, elderberry, fig";
    let out = dedup_comma_lists(input);
    assert_eq!(out, input);
}

#[test]
fn dedup_comma_list_consecutive() {
    assert_eq!(
        dedup_comma_lists("Runway, Runway, LeonardoAi, LeonardoAi"),
        "Runway, LeonardoAi"
    );
}

#[test]
fn dedup_comma_list_case_insensitive() {
    assert_eq!(
        dedup_comma_lists("Apple, apple, Banana, banana"),
        "Apple, Banana"
    );
}

#[test]
fn dedup_comma_list_no_dupes() {
    assert_eq!(dedup_comma_lists("A, B, C"), "A, B, C");
}

#[test]
fn dedup_comma_list_cycle_still_works() {
    assert_eq!(dedup_comma_lists("a, b, c, a, b, c, a, b, c"), "a, b, c");
}

// -- line-level dedup --

#[test]
fn dedup_lines_removes_repeated_lines_in_block() {
    let input = "Story A about product launch\nStory B about scaling\nStory A about product launch\nStory C about funding\nStory B about scaling";
    let out = dedup_lines(input);
    assert_eq!(
        out.matches("Story A about product launch").count(),
        1,
        "Duplicate line not removed: {out}"
    );
    assert_eq!(
        out.matches("Story B about scaling").count(),
        1,
        "Duplicate line not removed: {out}"
    );
    assert!(out.contains("Story C about funding"));
}

// -- trailing empty headings --

#[test]
fn empty_heading_at_eof_stripped() {
    let input = "Content\n\n## Support\n\n## Developers";
    let result = strip_trailing_empty_headings(input);
    assert!(!result.contains("## Support"));
    assert!(!result.contains("## Developers"));
}

#[test]
fn empty_heading_before_same_level_stripped() {
    let input = "## A\n\n## B\n\nContent here";
    let result = strip_trailing_empty_headings(input);
    assert!(!result.contains("## A"));
    assert!(result.contains("## B"));
    assert!(result.contains("Content here"));
}

#[test]
fn heading_with_subsection_preserved() {
    let input = "## Section\n\n### Subsection\n\nContent";
    assert_eq!(strip_trailing_empty_headings(input), input);
}

#[test]
fn heading_with_content_preserved() {
    let input = "## Features\n\nGreat stuff\n\n## More\n\nAlso great";
    assert_eq!(strip_trailing_empty_headings(input), input);
}

// -- empty code blocks --

#[test]
fn empty_code_block_stripped() {
    let input = "Before\n\n```\n\n```\n\nAfter";
    let result = strip_empty_code_blocks(input);
    assert!(!result.contains("```"));
    assert!(result.contains("Before"));
    assert!(result.contains("After"));
}

#[test]
fn empty_code_block_with_lang_stripped() {
    let input = "Text\n\n```js\n\n```\n\nMore";
    let result = strip_empty_code_blocks(input);
    assert!(!result.contains("```"));
}

#[test]
fn nonempty_code_block_preserved() {
    let input = "```\nconst x = 1;\n```";
    assert_eq!(strip_empty_code_blocks(input), input);
}
