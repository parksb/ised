use ised::highlight_match;
use regex::Regex;
use tui::text::Spans;

fn spans_to_string(spans: &Spans) -> String {
    spans
        .0
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>()
}

#[test]
fn test_highlight_no_match() {
    let input = "this line has no match";
    let re = Regex::new("not_found").ok();
    let result = highlight_match(input, &re);

    assert_eq!(result.len(), 1);
    let line = spans_to_string(&result[0]);
    assert_eq!(line, input);
}

#[test]
fn test_highlight_single_match() {
    let input = "match here please";
    let re = Regex::new("match").ok();
    let result = highlight_match(input, &re);

    let line = spans_to_string(&result[0]);
    assert_eq!(line, "match here please");

    // check highlighted section is styled
    assert_eq!(result[0].0.len(), 2); // "match" and the rest
    assert_eq!(result[0].0[0].content.as_ref(), "match");
    assert!(result[0].0[0].style.fg == Some(tui::style::Color::Green));
}

#[test]
fn test_highlight_multiple_matches() {
    let input = "repeat repeat repeat";
    let re = Regex::new("repeat").ok();
    let result = highlight_match(input, &re);

    let line = spans_to_string(&result[0]);
    assert_eq!(line, input);
    assert_eq!(
        result[0]
            .0
            .iter()
            .filter(|s| s.style.fg == Some(tui::style::Color::Green))
            .count(),
        3
    );
}

#[test]
fn test_highlight_partial_match() {
    let input = "only match part of this";
    let re = Regex::new("part").ok();
    let result = highlight_match(input, &re);

    let line = spans_to_string(&result[0]);
    assert!(line.contains("part"));
    assert!(result[0]
        .0
        .iter()
        .any(|s| s.content.as_ref() == "part" && s.style.fg == Some(tui::style::Color::Green)));
}
