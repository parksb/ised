use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::sync::mpsc;
use std::time::Duration;

use crate::app::App;

mod app;
mod config;
mod ui;
mod utils;

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let (tx, rx) = mpsc::channel();
    let mut app_clone = app.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            app_clone.load_files().await;
            let _ = tx.send(app_clone);
        });
    });

    let res: io::Result<()> = loop {
        if let Ok(new_app) = rx.try_recv() {
            app = new_app;
        }

        let filtered_files = app.filter_files();
        let file_content = if !app.is_loading {
            filtered_files
                .get(app.selected)
                .and_then(|file| std::fs::read_to_string(file).ok())
        } else {
            None
        };

        terminal.draw(|f| ui::draw(f, &app, &filtered_files, file_content))?;

        if crossterm::event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = crossterm::event::read()? {
                if app.handle_key_event(key, &filtered_files)? {
                    break Ok(());
                }
            }
        }

        app.spin();
    };

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err);
    }

    Ok(())
}
