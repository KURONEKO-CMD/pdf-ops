#![cfg(feature = "tui")]

use anyhow::{Context, Result};
use crossterm::{execute, terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{prelude::*, widgets::*};
use std::{io::stdout, path::PathBuf, sync::mpsc, thread, time::Duration, sync::{Arc, atomic::{AtomicBool, Ordering, AtomicU64}}};

use crate::scan::{self, ScanConfig, ScanEvent, CancelHandle};

struct FileItem {
    name: String,
    path: PathBuf,
    checked: bool,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Focus { Left, Right }

#[derive(Copy, Clone, PartialEq, Eq)]
enum InputMode { None, EditOutput, EditPages }

struct AppState {
    input_dir: PathBuf,
    files: Vec<FileItem>,
    selected: usize,
    status: String,
    scanning: bool,
    scan_depth: Option<usize>,
    cancel: Option<CancelHandle>,
    // selection/order panel
    order: Vec<usize>, // indexes into files
    order_selected: usize,
    focus: Focus,
    // run options
    force: bool,
    // job
    job_running: bool,
    // merge options
    output: PathBuf,
    pages: Option<String>,
    // input overlay
    input_mode: InputMode,
    input_buffer: String,
}

impl AppState {
    fn new(input_dir: PathBuf) -> Self {
        let output_default = input_dir.join("merged.pdf");
        Self {
            input_dir,
            files: Vec::new(),
            selected: 0,
            status: String::from("按 q 退出，Tab 切换面板，Space 选择，↑/↓ 导航，u/d 调整顺序，Enter 运行，r 重扫，[ / ] 深度，\\ 无限，F 覆盖"),
            scanning: true,
            scan_depth: Some(1),
            cancel: None,
            order: Vec::new(),
            order_selected: 0,
            focus: Focus::Left,
            force: false,
            job_running: false,
            output: output_default,
            pages: None,
            input_mode: InputMode::None,
            input_buffer: String::new(),
        }
    }
}

enum UiMsg {
    Found(PathBuf),
    Error(String),
    Done,
    Progress { pos: u64, len: u64, msg: String },
    JobDone(Result<()>, String),
}

struct TuiProgress {
    tx: mpsc::Sender<UiMsg>,
    len: AtomicU64,
    pos: AtomicU64,
}

impl TuiProgress {
    fn new(tx: mpsc::Sender<UiMsg>) -> Self { Self { tx, len: AtomicU64::new(0), pos: AtomicU64::new(0) } }
}

impl crate::progress::ProgressSink for TuiProgress {
    fn set_len(&self, len: u64) { self.len.store(len, Ordering::Relaxed); let _ = self.tx.send(UiMsg::Progress{ pos: self.pos.load(Ordering::Relaxed), len, msg: String::new() }); }
    fn inc(&self, n: u64) { let p = self.pos.fetch_add(n, Ordering::Relaxed) + n; let _ = self.tx.send(UiMsg::Progress{ pos: p, len: self.len.load(Ordering::Relaxed), msg: String::new() }); }
    fn set_message(&self, msg: std::borrow::Cow<'static, str>) { let _ = self.tx.send(UiMsg::Progress{ pos: self.pos.load(Ordering::Relaxed), len: self.len.load(Ordering::Relaxed), msg: msg.into_owned() }); }
    fn finish(&self, msg: std::borrow::Cow<'static, str>) { let _ = self.tx.send(UiMsg::Progress{ pos: self.len.load(Ordering::Relaxed), len: self.len.load(Ordering::Relaxed), msg: msg.into_owned() }); }
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
    spawn_scan(&mut app, tx.clone());

    // event loop
    loop {
        // handle channel messages
        while let Ok(msg) = rx.try_recv() {
            match msg {
                UiMsg::Found(p) => {
                    app.files.push(FileItem{ name: p.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string(), path: p, checked: false });
                    if app.selected >= app.files.len() { app.selected = app.files.len().saturating_sub(1); }
                }
                UiMsg::Error(e) => { app.status = format!("扫描失败: {}", e); }
                UiMsg::Done => { app.scanning = false; }
                UiMsg::Progress { pos, len, msg } => {
                    let msg_part = if msg.is_empty() { String::new() } else { format!(" · {}", msg) };
                    app.status = format!("进度: {}/{}{}", pos, len, msg_part);
                }
                UiMsg::JobDone(res, note) => {
                    app.job_running = false;
                    match res {
                        Ok(()) => app.status = format!("✅ 完成: {}", note),
                        Err(e) => app.status = format!("❌ 失败: {} · {}", note, e),
                    }
                }
            }
        }

        terminal.draw(|f| draw(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // input overlay handling
                if app.input_mode != InputMode::None {
                    match key.code {
                        KeyCode::Esc => { app.input_mode = InputMode::None; app.status = "已取消".into(); }
                        KeyCode::Enter => {
                            match app.input_mode {
                                InputMode::EditOutput => {
                                    if !app.input_buffer.is_empty() {
                                        app.output = PathBuf::from(app.input_buffer.clone());
                                    }
                                    app.status = format!("输出: {}", app.output.display());
                                }
                                InputMode::EditPages => {
                                    let trimmed = app.input_buffer.trim();
                                    if trimmed.is_empty() { app.pages = None; app.status = "清除页码范围".into(); }
                                    else { app.pages = Some(trimmed.to_string()); app.status = format!("页码: {}", trimmed); }
                                }
                                InputMode::None => {}
                            }
                            app.input_mode = InputMode::None;
                            app.input_buffer.clear();
                        }
                        KeyCode::Backspace => { app.input_buffer.pop(); }
                        KeyCode::Char(c) => { app.input_buffer.push(c); }
                        KeyCode::Tab => {}
                        _ => {}
                    }
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Tab => { app.focus = if app.focus==Focus::Left { Focus::Right } else { Focus::Left }; }
                    // navigation based on focus
                    KeyCode::Down | KeyCode::Char('j') => {
                        match app.focus {
                            Focus::Left => { app.selected = (app.selected + 1).min(app.files.len().saturating_sub(1)); }
                            Focus::Right => { app.order_selected = (app.order_selected + 1).min(app.order.len().saturating_sub(1)); }
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        match app.focus {
                            Focus::Left => { app.selected = app.selected.saturating_sub(1); }
                            Focus::Right => { app.order_selected = app.order_selected.saturating_sub(1); }
                        }
                    }
                    KeyCode::Char(' ') => {
                        if app.focus == Focus::Left {
                            if let Some(item) = app.files.get_mut(app.selected) {
                                item.checked = !item.checked;
                                if item.checked { app.order.push(app.selected); app.order_selected = app.order.len().saturating_sub(1); }
                                else { if let Some(pos) = app.order.iter().position(|&i| i==app.selected) { app.order.remove(pos); app.order_selected = app.order_selected.min(app.order.len().saturating_sub(1)); } }
                            }
                        }
                    }
                    // reorder in right panel
                    KeyCode::Char('u') if app.focus==Focus::Right => { if !app.order.is_empty() && app.order_selected>0 { let i=app.order_selected; app.order.swap(i-1,i); app.order_selected-=1; } }
                    KeyCode::Char('d') if app.focus==Focus::Right => { if !app.order.is_empty() && app.order_selected+1<app.order.len() { let i=app.order_selected; app.order.swap(i,i+1); app.order_selected+=1; } }
                    KeyCode::Char('U') if app.focus==Focus::Right => { if !app.order.is_empty() { let idx=app.order.remove(app.order_selected); app.order.insert(0, idx); app.order_selected=0; } }
                    KeyCode::Char('D') if app.focus==Focus::Right => { if !app.order.is_empty() { let idx=app.order.remove(app.order_selected); let last=app.order.len(); app.order.insert(last, idx); app.order_selected=last; } }
                    // rescan & depth
                    KeyCode::Char('r') => { rescan(&mut app, tx.clone()); }
                    KeyCode::Char(']') => { let next = match app.scan_depth { Some(d) => Some(d.saturating_add(1)), None => None }; app.scan_depth = next; rescan(&mut app, tx.clone()); }
                    KeyCode::Char('[') => { let next = match app.scan_depth { Some(d) if d>1 => Some(d-1), Some(_) => Some(1), None => Some(1) }; app.scan_depth = next; rescan(&mut app, tx.clone()); }
                    KeyCode::Char('\\') => { app.scan_depth = None; rescan(&mut app, tx.clone()); }
                    // force toggle
                    KeyCode::Char('F') => { app.force = !app.force; app.status = format!("覆盖: {}", if app.force {"开启"} else {"关闭"}); }
                    // edit options
                    KeyCode::Char('o') => { app.input_mode = InputMode::EditOutput; app.input_buffer = app.output.display().to_string(); app.status = "编辑输出路径: Enter 保存, Esc 取消".into(); }
                    KeyCode::Char('p') => { app.input_mode = InputMode::EditPages; app.input_buffer = app.pages.clone().unwrap_or_default(); app.status = "编辑页码范围(例: 1-3,5,10-): Enter 保存, Esc 取消".into(); }
                    // run merge job
                    KeyCode::Enter => {
                        if !app.job_running && !app.order.is_empty() {
                            app.job_running = true;
                            let files: Vec<PathBuf> = app.order.iter().filter_map(|&i| app.files.get(i)).map(|it| it.path.clone()).collect();
                            let output = if app.output.is_relative() { app.input_dir.join(&app.output) } else { app.output.clone() };
                            let force = app.force;
                            let pages = app.pages.clone();
                            let tx2 = tx.clone();
                            thread::spawn(move || {
                                let prog = TuiProgress::new(tx2.clone());
                                let res = crate::merge::run_with_files(&files, &output, pages.as_deref(), force, &prog);
                                let note = format!("{}", output.display());
                                let _ = tx2.send(UiMsg::JobDone(res, note));
                            });
                        }
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

fn spawn_scan(app: &mut AppState, tx: mpsc::Sender<UiMsg>) {
    // cancel previous
    if let Some(c) = &app.cancel { c.cancel(); }
    app.scanning = true;
    app.files.clear();
    app.selected = 0;
    let depth = app.scan_depth;
    let dir = app.input_dir.clone();
    let (rx, cancel) = scan::scan_stream(ScanConfig{
        input_dir: dir,
        includes: vec![], excludes: vec![], extra_exclude_paths: vec![],
        max_depth: depth, follow_links: false,
    });
    app.cancel = Some(cancel);
    // forward messages to UI channel
    thread::spawn(move || {
        while let Ok(ev) = rx.recv() {
            match ev {
                ScanEvent::Found(p) => { let _ = tx.send(UiMsg::Found(p)); }
                ScanEvent::Error(e) => { let _ = tx.send(UiMsg::Error(e)); }
                ScanEvent::Done => { let _ = tx.send(UiMsg::Done); break; }
            }
        }
    });
}

fn rescan(app: &mut AppState, tx: mpsc::Sender<UiMsg>) {
    app.status = "重新扫描中...".into();
    spawn_scan(app, tx);
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
    let depth = app.scan_depth.map(|d| d.to_string()).unwrap_or("∞".into());
    let pages = app.pages.clone().unwrap_or_else(|| "(全部)".into());
    let out_disp = if app.output.is_relative() { app.input_dir.join(&app.output) } else { app.output.clone() };
    let title = format!("pdf-ops · 输入: {} · 深度: {} · 选中: {} · 输出: {} · 页码: {}{}",
        app.input_dir.display(), depth, app.order.len(), out_disp.display(), pages,
        if app.scanning { " · 扫描中..." } else { "" }
    );
    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::ALL).title("Merge/Split"));
    f.render_widget(header, chunks[0]);

    // Main area: split into two columns
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    // Left list (all files)
    let items: Vec<ListItem> = app.files.iter().enumerate().map(|(_i, it)| {
        let mark = if it.checked { "[x]" } else { "[ ]" };
        let line = Line::from(format!("{} {}", mark, it.name));
        ListItem::new(line)
    }).collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Files"))
        .highlight_style(if app.focus==Focus::Left { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default().fg(Color::Cyan) })
        .highlight_symbol("▶ ");
    let mut state = ratatui::widgets::ListState::default();
    if !app.files.is_empty() { state.select(Some(app.selected)); }
    f.render_stateful_widget(list, main[0], &mut state);

    // Right list (selected/order)
    let sel_items: Vec<ListItem> = app.order.iter().enumerate().map(|(_pos, &idx)| {
        let name = app.files.get(idx).map(|f| f.name.clone()).unwrap_or_default();
        ListItem::new(Line::from(format!("{}", name)))
    }).collect();
    let sel_list = List::new(sel_items)
        .block(Block::default().borders(Borders::ALL).title("Selection / Order"))
        .highlight_style(if app.focus==Focus::Right { Style::default().fg(Color::Black).bg(Color::Green) } else { Style::default().fg(Color::Green) })
        .highlight_symbol("▶ ");
    let mut sel_state = ratatui::widgets::ListState::default();
    if !app.order.is_empty() { sel_state.select(Some(app.order_selected)); }
    f.render_stateful_widget(sel_list, main[1], &mut sel_state);

    // Status bar and optional input overlay
    let status = Paragraph::new(match app.input_mode { InputMode::None => app.status.clone(), _ => format!("{}: {}", app.status, app.input_buffer) });
    f.render_widget(status, chunks[2]);

    // Simple overlay box when in input mode
    if app.input_mode != InputMode::None {
        let area = centered_rect(60, 20, f.size());
        let prompt = Paragraph::new(app.input_buffer.clone())
            .block(Block::default().title(match app.input_mode { InputMode::EditOutput => "输出路径", InputMode::EditPages => "页码范围", InputMode::None => "" }).borders(Borders::ALL));
        f.render_widget(Clear, area);
        f.render_widget(prompt, area);
    }
}

fn centered_rect(pct_x: u16, pct_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(r);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(popup_layout[1]);
    horizontal[1]
}
