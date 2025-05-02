use ised::utils::highlight_diff_lines;
use ratatui::text::Line;

fn line_to_string(line: &Line) -> String {
    line.iter().map(|s| s.content.as_ref()).collect::<String>()
}

#[test]
fn test_diff_with_identical_lines() {
    let original = "same line\nidentical content".to_string();
    let replaced = "same line\nidentical content".to_string();

    let result = highlight_diff_lines(original.clone(), replaced.clone());
    assert_eq!(result.len(), 2);
    assert!(result
        .iter()
        .all(|line| line_to_string(line) == "same line"
            || line_to_string(line) == "identical content"));
}

#[test]
fn test_diff_with_single_replacement() {
    let original = "line 1\nchange me\nline 3".to_string();
    let replaced = "line 1\nchanged\nline 3".to_string();

    let result = highlight_diff_lines(original, replaced);
    assert_eq!(result.len(), 4);

    let lines: Vec<String> = result.iter().map(line_to_string).collect();
    assert!(lines.iter().any(|line| line.contains("- change me")));
    assert!(lines.iter().any(|line| line.contains("+ changed")));
}

#[test]
fn test_diff_with_removed_line() {
    let original = "keep this\nto be removed\nstay here".to_string();
    let replaced = "keep this\nstay here".to_string();

    let result = highlight_diff_lines(original, replaced);
    let lines: Vec<String> = result.iter().map(line_to_string).collect();
    assert!(lines.iter().any(|line| line.contains("- to be removed")));
    assert_eq!(lines.iter().filter(|l| l.contains("- ")).count(), 2);
}

#[test]
fn test_diff_with_added_line() {
    let original = "first line".to_string();
    let replaced = "first line\nnew line".to_string();

    let result = highlight_diff_lines(original, replaced);
    let lines: Vec<String> = result.iter().map(line_to_string).collect();
    assert!(lines.iter().any(|line| line.contains("+ new line")));
}
