use itertools::Itertools;
use regex::Regex;
use tui::{
    style::{Color, Modifier, Style},
    text::{Span, Spans},
};

pub fn highlight_match<'a>(text: &'a str, re: &Option<Regex>) -> Vec<Spans<'a>> {
    if let Some(re) = re {
        let mut spans = vec![];
        let mut last_end = 0;
        for mat in re.find_iter(text) {
            if mat.start() > last_end {
                spans.push(Span::raw(&text[last_end..mat.start()]));
            }
            spans.push(Span::styled(
                &text[mat.start()..mat.end()],
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            last_end = mat.end();
        }
        if last_end < text.len() {
            spans.push(Span::raw(&text[last_end..]));
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
