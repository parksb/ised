use itertools::Itertools;
use regex::{Captures, Regex};
use tui::{
    style::{Color, Modifier, Style},
    text::{Span, Spans},
};

pub fn highlight_match<'a>(text: &'a str, pattern: &str) -> Vec<Spans<'a>> {
    if let Some(index) = text.find(pattern) {
        let mut spans = vec![];
        if index > 0 {
            spans.push(Span::raw(&text[..index]));
        }
        spans.push(Span::styled(
            &text[index..index + pattern.len()],
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        if index + pattern.len() < text.len() {
            spans.push(Span::raw(&text[index + pattern.len()..]));
        }
        vec![Spans::from(spans)]
    } else {
        vec![Spans::from(Span::raw(text.to_string()))]
    }
}

pub fn highlight_diff_lines(original: String, replaced: String) -> Vec<Spans<'static>> {
    use itertools::EitherOrBoth::*;
    original
        .lines()
        .zip_longest(replaced.lines())
        .flat_map(|pair| match pair {
            Both(l, r) if l == r => vec![Spans::from(Span::raw(l.to_string()))],
            Both(l, r) => vec![
                Spans::from(vec![
                    Span::styled("- ".to_string(), Style::default().fg(Color::Red)),
                    Span::styled(l.to_string(), Style::default().fg(Color::Red)),
                ]),
                Spans::from(vec![
                    Span::styled("+ ".to_string(), Style::default().fg(Color::Green)),
                    Span::styled(r.to_string(), Style::default().fg(Color::Green)),
                ]),
            ],
            Left(l) => vec![Spans::from(vec![
                Span::styled("- ".to_string(), Style::default().fg(Color::Red)),
                Span::styled(l.to_string(), Style::default().fg(Color::Red)),
            ])],
            Right(r) => vec![Spans::from(vec![
                Span::styled("+ ".to_string(), Style::default().fg(Color::Green)),
                Span::styled(r.to_string(), Style::default().fg(Color::Green)),
            ])],
        })
        .collect()
}

pub fn apply_substitution_partial(
    content: &str,
    from_pattern: &str,
    to_replacement: &str,
) -> String {
    let re = Regex::new(from_pattern).unwrap_or_else(|_| Regex::new("$^").unwrap());

    re.replace_all(content, |caps: &Captures| {
        let mut replaced = to_replacement.to_string();
        for i in 1..caps.len() {
            let group_ref = format!("${}", i);
            replaced = replaced.replace(&group_ref, caps.get(i).map_or("", |m| m.as_str()));
        }
        replaced
    })
    .to_string()
}
