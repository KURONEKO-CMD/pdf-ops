#![cfg(feature = "tui")]

use anyhow::{Context, Result};
use crossterm::{execute, terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{prelude::*, widgets::*};
use std::{io::stdout, path::PathBuf, sync::mpsc, thread, time::Duration};

use crate::scan;

struct FileItem {
    name: String,
    path: PathBuf,
    checked: bool,
}

struct AppState {
    input_dir: PathBuf,
    files: Vec<FileItem>,
    selected: usize,
    status: String,
    scanning: bool,
}

impl AppState {
    fn new(input_dir: PathBuf) -> Self {
        Self { input_dir, files: Vec::new(), selected: 0, status: String::from("按 q 退出，Space 选择，↑/↓ 导航，r 重新扫描"), scanning: true }
    }
}

enum UiMsg {
    Files(Vec<PathBuf>),
    Error(String),
    Done,
}

pub fn run(_theme: Option<String>, _theme_file: Option<PathBuf>, input_dir: PathBuf) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(out);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let (tx, rx) = mpsc::channel::<UiMsg>();
    let mut app = AppState::new(input_dir);

    // spawn initial scan
    spawn_scan(app.input_dir.clone(), tx.clone());

    // event loop
    loop {
        // handle channel messages
        while let Ok(msg) = rx.try_recv() {
            match msg {
                UiMsg::Files(paths) => {
                    app.files = paths.into_iter().map(|p| FileItem {
                        name: p.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string(),
                        path: p,
                        checked: false,
                    }).collect();
                    if app.selected >= app.files.len() { app.selected = app.files.len().saturating_sub(1); }
                }
                UiMsg::Error(e) => { app.status = format!("扫描失败: {}", e); }
                UiMsg::Done => { app.scanning = false; }
            }
        }

        terminal.draw(|f| draw(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down | KeyCode::Char('j') => { app.selected = (app.selected + 1).min(app.files.len().saturating_sub(1)); }
                    KeyCode::Up | KeyCode::Char('k') => { app.selected = app.selected.saturating_sub(1); }
                    KeyCode::Char(' ') => {
                        if let Some(item) = app.files.get_mut(app.selected) { item.checked = !item.checked; }
                    }
                    KeyCode::Char('r') => {
                        app.scanning = true;
                        app.status = "重新扫描中...".into();
                        spawn_scan(app.input_dir.clone(), tx.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    let mut out2 = std::io::stdout();
    execute!(out2, LeaveAlternateScreen)?;
    Ok(())
}

fn spawn_scan(dir: PathBuf, tx: mpsc::Sender<UiMsg>) {
    thread::spawn(move || {
        // empty include/exclude; no extra excludes
        match scan::collect_pdfs(&dir, &[], &[], &[]) {
            Ok(mut v) => {
                // already sorted
                let _ = tx.send(UiMsg::Files(v));
                let _ = tx.send(UiMsg::Done);
            }
            Err(e) => { let _ = tx.send(UiMsg::Error(e.to_string())); let _ = tx.send(UiMsg::Done); }
        }
    });
}

fn draw(f: &mut ratatui::Frame<'_>, app: &AppState) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(1),    // main
            Constraint::Length(1), // status
        ])
        .split(size);

    // Header
    let title = format!("pdf-ops · 输入目录: {} · 文件数: {}{}", app.input_dir.display(), app.files.len(), if app.scanning { " · 扫描中..." } else { "" });
    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL).title("Merge/Split"));
    f.render_widget(header, chunks[0]);

    // Main list
    let items: Vec<ListItem> = app.files.iter().enumerate().map(|(i, it)| {
        let mark = if it.checked { "[x]" } else { "[ ]" };
        let line = Line::from(format!("{} {}", mark, it.name));
        ListItem::new(line)
    }).collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Files"))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("▶ ");
    let mut state = ratatui::widgets::ListState::default();
    if !app.files.is_empty() { state.select(Some(app.selected)); }
    f.render_stateful_widget(list, chunks[1], &mut state);

    // Status bar
    let status = Paragraph::new(app.status.clone());
    f.render_widget(status, chunks[2]);
}
