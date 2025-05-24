use ratatui::{
    layout::{Constraint, Direction, Layout, Position},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, ConfirmState, Focus};
use crate::utils::apply_substitution_partial;
use crate::utils::highlight_diff_lines;
use crate::utils::highlight_match;

fn safe_slice_chars(text: &str, start_char: usize, end_char: usize) -> &str {
    let char_indices: Vec<(usize, char)> = text.char_indices().collect();

    if char_indices.is_empty() || start_char >= char_indices.len() {
        return "";
    }

    let start_byte = char_indices[start_char].0;
    let end_byte = if end_char >= char_indices.len() {
        text.len()
    } else {
        char_indices[end_char].0
    };

    &text[start_byte..end_byte]
}

fn char_count(text: &str) -> usize {
    text.chars().count()
}

fn cursor_visual_position(text: &str, cursor_char_pos: usize) -> usize {
    text.chars()
        .take(cursor_char_pos)
        .map(|c| {
            match c {
            '\u{1100}'..='\u{11FF}' | // Hangul Jamo
            '\u{3040}'..='\u{309F}' | // Hiragana
            '\u{30A0}'..='\u{30FF}' | // Katakana
            '\u{3100}'..='\u{312F}' | // Bopomofo
            '\u{3200}'..='\u{32FF}' | // Enclosed CJK Letters and Months
            '\u{3400}'..='\u{4DBF}' | // CJK Unified Ideographs Extension A
            '\u{4E00}'..='\u{9FFF}' | // CJK Unified Ideographs
            '\u{A960}'..='\u{A97F}' | // Hangul Jamo Extended-A
            '\u{AC00}'..='\u{D7AF}' | // Hangul Syllables
            '\u{D7B0}'..='\u{D7FF}' | // Hangul Jamo Extended-B
            '\u{F900}'..='\u{FAFF}' | // CJK Compatibility Ideographs
            '\u{FE10}'..='\u{FE1F}' | // Vertical Forms
            '\u{FE30}'..='\u{FE4F}' | // CJK Compatibility Forms
            '\u{FF00}'..='\u{FFEF}' => 2, // Fullwidth forms
            _ => 1,
        }
        })
        .sum()
}

pub fn draw(f: &mut Frame, app: &mut App, filtered_files: &[String], file_content: Option<String>) {
    let size = f.area();
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(size);

    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(columns[0]);

    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(columns[1]);

    if app.is_loading {
        let loading_text = Paragraph::new(Text::from(format!("{} Loading files...", app.spinner)))
            .block(Block::default().title("File [L]ist").borders(Borders::ALL));
        f.render_widget(loading_text, left_rows[0]);
    } else {
        let list_height = left_rows[0].height as usize - 2;
        let mut offset = app.offset;
        if app.selected >= offset + list_height {
            offset = app.selected + 1 - list_height;
        } else if app.selected < offset {
            offset = app.selected;
        }

        let visible_files = filtered_files
            .iter()
            .skip(offset)
            .take(list_height)
            .enumerate()
            .map(|(i, fpath)| {
                let content = highlight_match(fpath, &app.filter_input);
                let mut item = ListItem::new(content);
                if i + offset == app.selected {
                    item = item.style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                item
            })
            .collect::<Vec<_>>();

        let file_list = List::new(visible_files).block(
            Block::default()
                .title("File [L]ist")
                .borders(Borders::ALL)
                .border_style(if app.focus == Focus::FileList {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }),
        );
        f.render_widget(file_list, left_rows[0]);
    }

    let filter_field_width = (left_rows[1].width.saturating_sub(2)) as usize;
    let from_field_width = (right_rows[1].width.saturating_sub(2)) as usize;
    let to_field_width = (right_rows[2].width.saturating_sub(2)) as usize;

    app.update_field_widths(filter_field_width, from_field_width, to_field_width);

    let filter_char_count = char_count(&app.filter_input);
    let mut filter_visible_text = "";
    let mut filter_end_char = app.filter_view_offset;

    if filter_char_count > app.filter_view_offset {
        // Calculate how many characters we can fit based on visual width
        let mut visual_width_used = 0;
        let chars: Vec<char> = app.filter_input.chars().collect();

        for (i, &char) in chars.iter().enumerate().skip(app.filter_view_offset) {
            let char_visual_width = match char {
                '\u{1100}'..='\u{11FF}'
                | '\u{3040}'..='\u{309F}'
                | '\u{30A0}'..='\u{30FF}'
                | '\u{3100}'..='\u{312F}'
                | '\u{3200}'..='\u{32FF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{4E00}'..='\u{9FFF}'
                | '\u{A960}'..='\u{A97F}'
                | '\u{AC00}'..='\u{D7AF}'
                | '\u{D7B0}'..='\u{D7FF}'
                | '\u{F900}'..='\u{FAFF}'
                | '\u{FE10}'..='\u{FE1F}'
                | '\u{FE30}'..='\u{FE4F}'
                | '\u{FF00}'..='\u{FFEF}' => 2,
                _ => 1,
            };

            if visual_width_used + char_visual_width > filter_field_width {
                break;
            }
            visual_width_used += char_visual_width;
            filter_end_char = i + 1;
        }

        filter_visible_text =
            safe_slice_chars(&app.filter_input, app.filter_view_offset, filter_end_char);
    }
    let filter_input = Paragraph::new(Text::from(filter_visible_text)).block(
        Block::default()
            .title("[G]lob Filter")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::FilePathFilter {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(filter_input, left_rows[1]);
    if app.focus == Focus::FilePathFilter {
        let cursor_x = if app.filter_cursor >= app.filter_view_offset {
            let visible_cursor_pos = app.filter_cursor - app.filter_view_offset;
            cursor_visual_position(filter_visible_text, visible_cursor_pos)
        } else {
            0
        };
        f.set_cursor_position(Position::new(
            left_rows[1].x + 1 + cursor_x as u16,
            left_rows[1].y + 1,
        ));
    }

    let blank_text = match &app.confirm {
        ConfirmState::Confirming(path) => format!("Apply changes to {}? (y/n)", path),
        ConfirmState::ConfirmingAll(_) => "Apply changes to ALL files? (y/n)".to_string(),
        ConfirmState::None => "".to_string(),
    };
    let blank = Paragraph::new(Text::from(blank_text));
    f.render_widget(blank, left_rows[2]);

    let diff_output = if let Some(content) = file_content {
        let replaced = apply_substitution_partial(&content, &app.from_input, &app.to_input);
        highlight_diff_lines(content, replaced)
    } else {
        vec![Line::from("No file selected.")]
    };

    let height = right_rows[0].height as usize - 2;
    let visible_diff = diff_output
        .into_iter()
        .skip(app.diff_scroll)
        .take(height)
        .collect::<Vec<_>>();

    let diff_view = Paragraph::new(visible_diff).block(
        Block::default()
            .title("[D]iff")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::DiffView {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(diff_view, right_rows[0]);

    let from_char_count = char_count(&app.from_input);
    let mut from_visible_text = "";
    let mut from_end_char = app.from_view_offset;

    if from_char_count > app.from_view_offset {
        // Calculate how many characters we can fit based on visual width
        let mut visual_width_used = 0;
        let chars: Vec<char> = app.from_input.chars().collect();

        for (i, &char) in chars.iter().enumerate().skip(app.from_view_offset) {
            let char_visual_width = match char {
                '\u{1100}'..='\u{11FF}'
                | '\u{3040}'..='\u{309F}'
                | '\u{30A0}'..='\u{30FF}'
                | '\u{3100}'..='\u{312F}'
                | '\u{3200}'..='\u{32FF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{4E00}'..='\u{9FFF}'
                | '\u{A960}'..='\u{A97F}'
                | '\u{AC00}'..='\u{D7AF}'
                | '\u{D7B0}'..='\u{D7FF}'
                | '\u{F900}'..='\u{FAFF}'
                | '\u{FE10}'..='\u{FE1F}'
                | '\u{FE30}'..='\u{FE4F}'
                | '\u{FF00}'..='\u{FFEF}' => 2,
                _ => 1,
            };

            if visual_width_used + char_visual_width > from_field_width {
                break;
            }
            visual_width_used += char_visual_width;
            from_end_char = i + 1;
        }

        from_visible_text = safe_slice_chars(&app.from_input, app.from_view_offset, from_end_char);
    }
    let from_paragraph = Paragraph::new(Text::from(from_visible_text)).block(
        Block::default()
            .title("[F]rom")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::From {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(from_paragraph, right_rows[1]);
    if app.focus == Focus::From {
        let cursor_x = if app.from_cursor >= app.from_view_offset {
            let visible_cursor_pos = app.from_cursor - app.from_view_offset;
            cursor_visual_position(from_visible_text, visible_cursor_pos)
        } else {
            0
        };
        f.set_cursor_position(Position::new(
            right_rows[1].x + 1 + cursor_x as u16,
            right_rows[1].y + 1,
        ));
    }

    let to_char_count = char_count(&app.to_input);
    let mut to_visible_text = "";
    let mut to_end_char = app.to_view_offset;

    if to_char_count > app.to_view_offset {
        // Calculate how many characters we can fit based on visual width
        let mut visual_width_used = 0;
        let chars: Vec<char> = app.to_input.chars().collect();

        for (i, &char) in chars.iter().enumerate().skip(app.to_view_offset) {
            let char_visual_width = match char {
                '\u{1100}'..='\u{11FF}'
                | '\u{3040}'..='\u{309F}'
                | '\u{30A0}'..='\u{30FF}'
                | '\u{3100}'..='\u{312F}'
                | '\u{3200}'..='\u{32FF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{4E00}'..='\u{9FFF}'
                | '\u{A960}'..='\u{A97F}'
                | '\u{AC00}'..='\u{D7AF}'
                | '\u{D7B0}'..='\u{D7FF}'
                | '\u{F900}'..='\u{FAFF}'
                | '\u{FE10}'..='\u{FE1F}'
                | '\u{FE30}'..='\u{FE4F}'
                | '\u{FF00}'..='\u{FFEF}' => 2,
                _ => 1,
            };

            if visual_width_used + char_visual_width > to_field_width {
                break;
            }
            visual_width_used += char_visual_width;
            to_end_char = i + 1;
        }

        to_visible_text = safe_slice_chars(&app.to_input, app.to_view_offset, to_end_char);
    }
    let to_paragraph = Paragraph::new(Text::from(to_visible_text)).block(
        Block::default()
            .title("[T]o")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::To {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(to_paragraph, right_rows[2]);
    if app.focus == Focus::To {
        let cursor_x = if app.to_cursor >= app.to_view_offset {
            let visible_cursor_pos = app.to_cursor - app.to_view_offset;
            cursor_visual_position(to_visible_text, visible_cursor_pos)
        } else {
            0
        };
        f.set_cursor_position(Position::new(
            right_rows[2].x + 1 + cursor_x as u16,
            right_rows[2].y + 1,
        ));
    }
}
