use ised::utils::highlight_match;
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
    let pattern = "not_found";
    let result = highlight_match(input, pattern);

    assert_eq!(result.len(), 1);
    let line = spans_to_string(&result[0]);
    assert_eq!(line, input);

    assert!(result[0].0.iter().all(|s| s.style.fg.is_none()));
}

#[test]
fn test_highlight_single_match() {
    let input = "match here please";
    let pattern = "match";
    let result = highlight_match(input, pattern);

    let line = spans_to_string(&result[0]);
    assert_eq!(line, input);

    assert_eq!(result[0].0.len(), 2);

    let first = &result[0].0[0];
    assert_eq!(first.content.as_ref(), "match");
    assert_eq!(first.style.fg, Some(tui::style::Color::Green));
}

#[test]
fn test_highlight_multiple_matches_only_first() {
    let input = "repeat repeat repeat";
    let pattern = "repeat";
    let result = highlight_match(input, pattern);

    let line = spans_to_string(&result[0]);
    assert_eq!(line, input);

    assert_eq!(
        result[0]
            .0
            .iter()
            .filter(|s| s.style.fg == Some(tui::style::Color::Green))
            .count(),
        1
    );
}

#[test]
fn test_highlight_partial_match() {
    let input = "only match part of this";
    let pattern = "part";
    let result = highlight_match(input, pattern);

    let line = spans_to_string(&result[0]);
    assert!(line.contains("part"));
    assert!(result[0]
        .0
        .iter()
        .any(|s| s.content.as_ref() == "part" && s.style.fg == Some(tui::style::Color::Green)));
}
