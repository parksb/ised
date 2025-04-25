use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use regex::Regex;
use std::{fs, io};
use tui::backend::Backend;
use tui::Terminal;

use crate::ui;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Focus {
    FileList,
    FilePathFilter,
    DiffView,
    From,
    To,
}

pub enum ConfirmState {
    None,
    Confirming(String),
    ConfirmingAll(Vec<String>),
}

pub struct App {
    pub files: Vec<String>,
    pub selected: usize,
    pub offset: usize,
    pub filter_input: String,
    pub from_input: String,
    pub to_input: String,
    pub focus: Focus,
    pub diff_scroll: usize,
    pub confirm: ConfirmState,
}

impl App {
    pub fn new() -> Self {
        let files: Vec<String> = walkdir::WalkDir::new(".")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().display().to_string())
            .collect();

        App {
            files,
            selected: 0,
            offset: 0,
            filter_input: String::new(),
            from_input: String::new(),
            to_input: String::new(),
            focus: Focus::FileList,
            diff_scroll: 0,
            confirm: ConfirmState::None,
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            let filter_re = Regex::new(&self.filter_input).ok();
            let filtered_files = self.filter_files(&filter_re);

            terminal.draw(|f| ui::draw(f, self, &filtered_files, &filter_re))?;

            if event::poll(std::time::Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key_event(key, &filtered_files)? {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn filter_files(&self, re: &Option<Regex>) -> Vec<String> {
        self.files
            .iter()
            .filter(|f| re.as_ref().map_or(true, |re| re.is_match(f)))
            .cloned()
            .collect()
    }

    fn handle_key_event(&mut self, key: KeyEvent, filtered_files: &[String]) -> io::Result<bool> {
        match key {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => return Ok(true),

            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                self.focus = match self.focus {
                    Focus::FileList => Focus::FilePathFilter,
                    Focus::FilePathFilter => Focus::DiffView,
                    Focus::DiffView => Focus::From,
                    Focus::From => Focus::To,
                    Focus::To => Focus::FileList,
                };
            }

            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.focus == Focus::FileList {
                    self.confirm = ConfirmState::ConfirmingAll(filtered_files.to_vec());
                }
            }

            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if self.focus == Focus::FileList {
                    if let Some(file) = filtered_files.get(self.selected) {
                        self.confirm = ConfirmState::Confirming(file.clone());
                    }
                }
            }

            KeyEvent {
                code: KeyCode::Char('y'),
                ..
            } => match &self.confirm {
                ConfirmState::Confirming(path) => {
                    self.apply_substitution(path)?;
                    self.confirm = ConfirmState::None;
                }
                ConfirmState::ConfirmingAll(paths) => {
                    for path in paths {
                        let _ = self.apply_substitution(path);
                    }
                    self.confirm = ConfirmState::None;
                }
                ConfirmState::None => self.push_input('y'),
            },

            KeyEvent {
                code: KeyCode::Char('n'),
                ..
            } => {
                if !matches!(self.confirm, ConfirmState::None) {
                    self.confirm = ConfirmState::None;
                } else {
                    self.push_input('n');
                }
            }

            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.confirm = ConfirmState::None;
            }

            KeyEvent {
                code: KeyCode::Up, ..
            } => match self.focus {
                Focus::FileList => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                }
                Focus::DiffView => {
                    self.diff_scroll = self.diff_scroll.saturating_sub(1);
                }
                _ => {}
            },

            KeyEvent {
                code: KeyCode::Down,
                ..
            } => match self.focus {
                Focus::FileList => {
                    if self.selected + 1 < filtered_files.len() {
                        self.selected += 1;
                    }
                }
                Focus::DiffView => {
                    self.diff_scroll += 1;
                }
                _ => {}
            },

            KeyEvent {
                code: KeyCode::Char(c),
                ..
            } => match c {
                'j' => match self.focus {
                    Focus::FileList => {
                        if self.selected + 1 < filtered_files.len() {
                            self.selected += 1;
                        }
                    }
                    Focus::DiffView => self.diff_scroll += 1,
                    _ => self.push_input('j'),
                },
                'k' => match self.focus {
                    Focus::FileList => self.selected = self.selected.saturating_sub(1),
                    Focus::DiffView => self.diff_scroll = self.diff_scroll.saturating_sub(1),
                    _ => self.push_input('k'),
                },
                _ => self.push_input(c),
            },

            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => match self.focus {
                Focus::FilePathFilter => {
                    self.filter_input.pop();
                    self.selected = 0;
                    self.offset = 0;
                }
                Focus::From => {
                    self.from_input.pop();
                }
                Focus::To => {
                    self.to_input.pop();
                }
                _ => {}
            },

            _ => {}
        }
        Ok(false)
    }

    fn push_input(&mut self, c: char) {
        match self.focus {
            Focus::FilePathFilter => self.filter_input.push(c),
            Focus::From => self.from_input.push(c),
            Focus::To => self.to_input.push(c),
            _ => {}
        }
    }

    fn apply_substitution(&self, path: &str) -> io::Result<()> {
        let content = fs::read_to_string(path)?;
        let re = Regex::new(&self.from_input).unwrap_or_else(|_| Regex::new("$^").unwrap());
        let replaced = re.replace_all(&content, self.to_input.as_str()).to_string();
        fs::write(path, replaced)
    }
}
