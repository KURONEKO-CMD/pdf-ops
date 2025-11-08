#![cfg(feature = "tui")]

use anyhow::Result;
use crossterm::{execute, terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{prelude::*, widgets::*};
use std::{io::stdout, path::PathBuf, sync::mpsc, thread, time::Duration, sync::{atomic::{Ordering, AtomicU64}}};
use crate::pathutil::sanitize_path_input;

use crate::scan::{self, ScanConfig, ScanEvent, CancelHandle};
mod theme;
use theme::Theme;
use lopdf;

struct FileItem {
    name: String,
    path: PathBuf,
    checked: bool,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Focus { Left, Right }

#[derive(Copy, Clone, PartialEq, Eq)]
enum InputMode { None, EditOutput, EditPages, PickMode, FilesMenu, EditInput, PickDepth, OptionsMenu, PickOverwrite, EditSplitSuffix, EditSplitRange, ConfirmLarge, Help }

#[derive(Copy, Clone, PartialEq, Eq)]
enum Mode { Merge, Split }

#[derive(Copy, Clone, PartialEq, Eq)]
enum OverwritePolicy { Force, Suffix }

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
    top_focus: bool,
    top_index: usize,
    mode: Mode,
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
    input_cursor: usize,
    mode_pick_index: usize,
    files_menu_index: usize,
    depth_pick_index: usize,
    options_menu_index: usize,
    overwrite_pick_index: usize,
    theme: Theme,
    output_auto_follow: bool,
    overwrite_policy: OverwritePolicy,
    split_suffix: String,
    split_group: usize,
    // pending split confirmation
    pend_input: Option<PathBuf>,
    pend_out_dir: Option<PathBuf>,
    pend_ranges: Option<String>,
    pend_each: bool,
    pend_expected: usize,
}

impl AppState {
    fn new(input_dir: PathBuf) -> Self {
        let output_default = input_dir.join("merged.pdf");
        Self {
            input_dir,
            files: Vec::new(),
            selected: 0,
            status: String::from("Quit: q  Focus: Tab  Select: Space  Move: ↑/↓/j/k  Reorder: u/d/U/D  Rescan: r  Depth: [ ] \\  Output: o  Pages: p  Force: F  Run: Enter"),
            scanning: true,
            scan_depth: Some(1),
            cancel: None,
            order: Vec::new(),
            order_selected: 0,
            focus: Focus::Left,
            top_focus: false,
            top_index: 0,
            mode: Mode::Merge,
            force: false,
            job_running: false,
            output: output_default,
            pages: None,
            input_mode: InputMode::None,
            input_buffer: String::new(),
            input_cursor: 0,
            mode_pick_index: 0,
            files_menu_index: 0,
            depth_pick_index: 0,
            options_menu_index: 0,
            overwrite_pick_index: 1, // default to Suffix
            theme: Theme::gitui_dark(),
            output_auto_follow: true,
            overwrite_policy: OverwritePolicy::Suffix,
            split_suffix: "_{index}".into(),
            split_group: 1,
            pend_input: None,
            pend_out_dir: None,
            pend_ranges: None,
            pend_each: true,
            pend_expected: 0,
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
    app.status = "Ready".into();

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
                UiMsg::Error(e) => { app.status = format!("Scan error: {}", e); }
                UiMsg::Done => { app.scanning = false; }
                UiMsg::Progress { pos, len, msg } => {
                    let msg_part = if msg.is_empty() { String::new() } else { format!(" · {}", msg) };
                    app.status = format!("Progress: {}/{}{}", pos, len, msg_part);
                }
                UiMsg::JobDone(res, note) => {
                    app.job_running = false;
                    match res {
                        Ok(()) => app.status = format!("✓ Done: {}", note),
                        Err(e) => app.status = format!("× Failed: {} · {}", note, e),
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
                        KeyCode::Esc => { app.input_mode = InputMode::None; app.status = "Canceled".into(); app.input_buffer.clear(); app.input_cursor = 0; }
                        KeyCode::Enter => {
                            match app.input_mode {
                                InputMode::EditOutput => {
                                    let norm = sanitize_path_input(&app.input_buffer);
                                    app.output = PathBuf::from(norm);
                                    app.status = format!("Output: {}", app.output.display());
                                }
                                InputMode::EditPages => {
                                    let trimmed = app.input_buffer.trim();
                                    if trimmed.is_empty() { app.pages = None; app.status = "Clear page ranges".into(); }
                                    else { app.pages = Some(trimmed.to_string()); app.status = format!("Pages: {}", trimmed); }
                                }
                                InputMode::PickMode => {
                                    app.mode = if app.mode_pick_index==0 { Mode::Merge } else { Mode::Split };
                                    app.status = format!("Mode: {}", match app.mode { Mode::Merge=>"Merge", Mode::Split=>"Split"});
                                }
                                InputMode::FilesMenu => {
                                    match app.files_menu_index {
                                        0 => { // Input Path
                                            app.input_mode = InputMode::EditInput;
                                            app.input_buffer = app.input_dir.display().to_string();
                                            app.input_cursor = app.input_buffer.len();
                                            app.status = "Edit input path: Enter to save, Esc to cancel".into();
                                            continue;
                                        }
                                        1 => { // Output Path
                                            app.input_mode = InputMode::EditOutput;
                                            app.input_buffer = app.output.display().to_string();
                                            app.input_cursor = app.input_buffer.len();
                                            app.status = "Edit output path: Enter to save, Esc to cancel".into();
                                            continue;
                                        }
                                        _ => {}
                                    }
                                }
                                InputMode::EditInput => {
                                    let norm = sanitize_path_input(&app.input_buffer);
                                    let new_dir = PathBuf::from(norm);
                                    app.input_dir = new_dir;
                                    app.order.clear();
                                    app.selected = 0;
                                    if app.output_auto_follow { app.output = PathBuf::from("merged.pdf"); }
                                    app.status = format!("Input: {}", app.input_dir.display());
                                    rescan(&mut app, tx.clone());
                                }
                                InputMode::PickDepth => {
                                    app.scan_depth = match app.depth_pick_index { 0 => Some(1), 1 => Some(2), 2 => Some(3), _ => None };
                                    let label = match app.scan_depth { Some(d)=>d.to_string(), None=>"∞".into() };
                                    app.status = format!("Depth: {}", label);
                                    rescan(&mut app, tx.clone());
                                }
                                InputMode::OptionsMenu => {
                                    match app.options_menu_index {
                                        0 => { // Depth
                                            app.input_mode = InputMode::PickDepth;
                                            app.depth_pick_index = match app.scan_depth { Some(1) => 0, Some(2) => 1, Some(3) => 2, None => 3, _ => 0 };
                                            app.status = "Pick scan depth: 1 / 2 / 3 / ∞".into();
                                            continue;
                                        }
                                        1 => { // Output auto-follow toggle
                                            app.output_auto_follow = !app.output_auto_follow;
                                            app.status = format!("Output auto-follow: {}", if app.output_auto_follow {"On"} else {"Off"});
                                        }
                                        2 => { // Overwrite policy
                                            app.input_mode = InputMode::PickOverwrite;
                                            app.overwrite_pick_index = if matches!(app.overwrite_policy, OverwritePolicy::Force) {0} else {1};
                                            app.status = "Pick overwrite: Force / Suffix".into();
                                            continue;
                                        }
                                        3 => { // Split range
                                            app.input_mode = InputMode::EditSplitRange;
                                            app.input_buffer = app.split_group.to_string();
                                            app.input_cursor = app.input_buffer.len();
                                            app.status = "Edit split range (pages per file, >=1)".into();
                                            continue;
                                        }
                                        4 => { // Split suffix
                                            app.input_mode = InputMode::EditSplitSuffix;
                                            app.input_buffer = app.split_suffix.clone();
                                            app.input_cursor = app.input_buffer.len();
                                            app.status = "Edit split suffix (use {index}): Enter to save, Esc to cancel".into();
                                            continue;
                                        }
                                        _ => {}
                                    }
                                }
                                InputMode::EditSplitRange => {
                                    let v = app.input_buffer.trim().parse::<usize>().unwrap_or(1).max(1);
                                    app.split_group = v;
                                    app.status = format!("Split range: {}", app.split_group);
                                }
                                InputMode::PickOverwrite => {
                                    app.overwrite_policy = if app.overwrite_pick_index==0 { OverwritePolicy::Force } else { OverwritePolicy::Suffix };
                                    app.status = format!("Overwrite: {}", match app.overwrite_policy { OverwritePolicy::Force=>"Force", OverwritePolicy::Suffix=>"Suffix" });
                                }
                                InputMode::EditSplitSuffix => {
                                    app.split_suffix = app.input_buffer.clone();
                                    app.status = format!("Split suffix: {}", app.split_suffix);
                                }
                                InputMode::ConfirmLarge => { /* Enter = no-op (prefer y/N) */ }
                                InputMode::Help => { /* Enter closes help; handled after this match */ }
                                InputMode::None => {}
                            }
                            app.input_mode = InputMode::None;
                            app.input_buffer.clear();
                            app.input_cursor = 0;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            match app.input_mode {
                                InputMode::PickMode => { app.mode_pick_index = (app.mode_pick_index+1).min(1); }
                                InputMode::FilesMenu => { app.files_menu_index = (app.files_menu_index+1).min(1); }
                                InputMode::PickDepth => { app.depth_pick_index = (app.depth_pick_index+1).min(2); }
                                InputMode::OptionsMenu => { app.options_menu_index = (app.options_menu_index+1).min(4); }
                                InputMode::PickOverwrite => { app.overwrite_pick_index = (app.overwrite_pick_index+1).min(1); }
                                _ => {}
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            match app.input_mode {
                                InputMode::PickMode => { if app.mode_pick_index>0 { app.mode_pick_index-=1; } }
                                InputMode::FilesMenu => { if app.files_menu_index>0 { app.files_menu_index-=1; } }
                                InputMode::PickDepth => { if app.depth_pick_index>0 { app.depth_pick_index-=1; } }
                                InputMode::OptionsMenu => { if app.options_menu_index>0 { app.options_menu_index-=1; } }
                                InputMode::PickOverwrite => { if app.overwrite_pick_index>0 { app.overwrite_pick_index-=1; } }
                                _ => {}
                            }
                        }
                        KeyCode::Left => { if app.input_cursor>0 { app.input_cursor-=1; } }
                        KeyCode::Right => { if app.input_cursor < app.input_buffer.len() { app.input_cursor+=1; } }
                        KeyCode::Home => { app.input_cursor = 0; }
                        KeyCode::End => { app.input_cursor = app.input_buffer.len(); }
                        KeyCode::Backspace => { if app.input_cursor>0 { app.input_buffer.remove(app.input_cursor-1); app.input_cursor-=1; } }
                        KeyCode::Delete => { if app.input_cursor < app.input_buffer.len() { app.input_buffer.remove(app.input_cursor); } }
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            if matches!(app.input_mode, InputMode::ConfirmLarge) {
                                if let (Some(inp), Some(outd)) = (app.pend_input.clone(), app.pend_out_dir.clone()) {
                                    let pattern = format!("{{base}}{}.pdf", app.split_suffix);
                                    let force = matches!(app.overwrite_policy, OverwritePolicy::Force) || app.force;
                                    let ranges = app.pend_ranges.clone();
                                    let each = app.pend_each;
                                    app.input_mode = InputMode::None;
                                    app.pend_input=None; app.pend_out_dir=None; app.pend_ranges=None; app.pend_expected=0; app.pend_each=true;
                                    spawn_split_job_params(inp, outd, each, ranges, pattern, force, tx.clone());
                                }
                            } else { app.input_buffer.insert(app.input_cursor, 'y'); app.input_cursor+=1; }
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            if matches!(app.input_mode, InputMode::ConfirmLarge) {
                                app.input_mode = InputMode::None; app.pend_input=None; app.pend_out_dir=None; app.pend_ranges=None; app.pend_expected=0; app.pend_each=true; app.status = "Canceled".into();
                            } else { app.input_buffer.insert(app.input_cursor, 'n'); app.input_cursor+=1; }
                        }
                        KeyCode::Char(c) => { app.input_buffer.insert(app.input_cursor, c); app.input_cursor+=1; }
                        KeyCode::Tab => {}
                        _ => {}
                    }
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => { if app.top_focus { app.top_focus = false; } else { app.status = "Canceled".into(); } }
                    KeyCode::Tab => {
                        let items_len = 4; // Files, Mode, Options, Help
                        if app.top_focus { app.top_index = (app.top_index+1)%items_len; }
                        else { app.focus = if app.focus==Focus::Left { Focus::Right } else { Focus::Left }; }
                    }
                    KeyCode::Char('g') => { app.top_focus = !app.top_focus; }
                    KeyCode::Left | KeyCode::Char('h') => { if app.top_focus && app.top_index>0 { app.top_index-=1; } }
                    KeyCode::Right | KeyCode::Char('l') => { if app.top_focus { let items_len = 4; app.top_index=(app.top_index+1)%items_len; } }
                    // Enter: open pickers at top; otherwise run job by mode
                    KeyCode::Enter => {
                        if app.top_focus {
                            if app.top_index==1 {
                                app.input_mode = InputMode::PickMode;
                                app.mode_pick_index = if matches!(app.mode, Mode::Merge) {0} else {1};
                                app.status = "Pick mode: Merge / Split · Enter=Confirm · Esc=Cancel".into();
                            } else if app.top_index==0 {
                                app.input_mode = InputMode::FilesMenu;
                                app.files_menu_index = 0;
                                app.status = "Files: Input Path / Output Path".into();
                            } else if app.top_index==2 {
                                app.input_mode = InputMode::OptionsMenu;
                                app.options_menu_index = 0;
                                app.status = "Options: Depth / Output auto-follow / Overwrite / Split suffix".into();
                            } else if app.top_index==3 {
                                app.input_mode = InputMode::Help;
                                app.input_buffer.clear();
                                app.status = "Help".into();
                            }
                        } else {
                            if !app.job_running && !app.order.is_empty() {
                                match app.mode {
                                    Mode::Merge => spawn_merge_job(&mut app, tx.clone()),
                                    Mode::Split => {
                                        // preflight: compute groups and expected count
                                        if let Some(first) = app.order.iter().filter_map(|&i| app.files.get(i)).map(|it| it.path.clone()).next() {
                                            let out_dir = choose_out_dir(&app.input_dir, &app.output);
                                            let group = app.split_group.max(1);
                                            let pages = match lopdf::Document::load(&first) { Ok(d)=> d.get_pages().len(), Err(_)=>0 };
                                            let (each, ranges, expected) = if group<=1 { (true, None, pages) } else {
                                                let ranges = make_ranges_spec(pages, group);
                                                let expected = (pages + group - 1)/group;
                                                (false, Some(ranges), expected)
                                            };
                                            if expected>20 {
                                                app.pend_input = Some(first);
                                                app.pend_out_dir = Some(out_dir);
                                                app.pend_ranges = ranges;
                                                app.pend_each = each;
                                                app.pend_expected = expected;
                                                app.input_mode = InputMode::ConfirmLarge;
                                                app.status = format!("This will create {} files. Proceed? (y/N)", app.pend_expected);
                                            } else {
                                                let pattern = format!("{{base}}{}.pdf", app.split_suffix);
                                                let force = matches!(app.overwrite_policy, OverwritePolicy::Force) || app.force;
                                                spawn_split_job_params(first, out_dir, each, ranges, pattern, force, tx.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
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
                    // rescan only (depth moved to Options)
                    KeyCode::Char('r') => { rescan(&mut app, tx.clone()); }
                    // force toggle
                    KeyCode::Char('F') => { app.force = !app.force; app.status = format!("Force overwrite: {}", if app.force {"On"} else {"Off"}); }
                    // edit options (Output path moved to Files menu)
                    KeyCode::Char('p') => { app.input_mode = InputMode::EditPages; app.input_buffer = app.pages.clone().unwrap_or_default(); app.status = "Edit page ranges (e.g., 1-3,5,10-): Enter to save, Esc to cancel".into(); }
                    // run merge job（另一路径已覆盖 Enter 触发）
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
    app.cancel = Some(cancel.clone());
    // forward messages to UI channel，若长时间无结果则自动取消释放资源
    thread::spawn(move || {
        use std::sync::mpsc::RecvTimeoutError;
        use std::time::{Duration, Instant};
        let timeout = Duration::from_secs(10);
        let mut last = Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(ev) => {
                    match ev {
                        ScanEvent::Found(p) => { last = Instant::now(); let _ = tx.send(UiMsg::Found(p)); }
                        ScanEvent::Error(e) => { let _ = tx.send(UiMsg::Error(e)); }
                        ScanEvent::Done => { let _ = tx.send(UiMsg::Done); break; }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if last.elapsed() >= timeout {
                        cancel.cancel();
                        let _ = tx.send(UiMsg::Error("Scan timeout, canceled to free resources".into()));
                        // 继续等待 Done 来收尾
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

fn rescan(app: &mut AppState, tx: mpsc::Sender<UiMsg>) {
    app.status = "Rescanning...".into();
    spawn_scan(app, tx);
}

fn draw(f: &mut ratatui::Frame<'_>, app: &AppState) {
    let size = f.size();
    // 全局背景填充为主题色
    let bg = Block::default().style(Style::default().bg(app.theme.bg).fg(app.theme.fg));
    f.render_widget(bg, size);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // menu + info 两个块
            Constraint::Min(1),    // main
            Constraint::Length(3), // status + help1 + help2
        ])
        .split(size);
    // 顶部：拆为 Menu 与 Info 两个块
    let top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(chunks[0]);

    // Menu（只显示菜单项，高对比度、加粗）
    let items = ["Files", "Mode", "Options", "Help"];
    let mut spans: Vec<Span> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let label = if app.top_focus && app.top_index==i { format!("[{}]", it) } else { it.to_string() };
        if i>0 { spans.push(Span::raw("  ")); }
        let style = if app.top_focus && app.top_index==i {
            Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.fg).add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(label, style));
    }
    let menu_para = Paragraph::new(Line::from(spans))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border).add_modifier(Modifier::BOLD))
            .title(Span::styled("Menu", Style::default().add_modifier(Modifier::BOLD))));
    f.render_widget(menu_para, top[0]);

    // Info（去掉前缀；高对比度、加粗、左对齐）
    let depth = app.scan_depth.map(|d| d.to_string()).unwrap_or("∞".into());
    let pages = app.pages.clone().unwrap_or_else(|| "(all)".into());
    let out_disp = if app.output.is_relative() { app.input_dir.join(&app.output) } else { app.output.clone() };
    let info = format!("Input: {} · Depth: {} · Selected: {} · Output: {} · Pages: {} · Mode: {}{}",
        app.input_dir.display(), depth, app.order.len(), out_disp.display(), pages,
        match app.mode { Mode::Merge=>"Merge", Mode::Split=>"Split" },
        if app.scanning { " · Scanning..." } else { "" }
    );
    let info_para = Paragraph::new(info)
        .style(Style::default().fg(app.theme.fg).add_modifier(Modifier::BOLD))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border).add_modifier(Modifier::BOLD))
            .title(Span::styled("Info", Style::default().add_modifier(Modifier::BOLD))));
    f.render_widget(info_para, top[1]);

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
        .style(Style::default().fg(app.theme.fg).add_modifier(Modifier::BOLD))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border).add_modifier(Modifier::BOLD))
            .title(Span::styled("Files", Style::default().add_modifier(Modifier::BOLD))))
        .highlight_style(if app.focus==Focus::Left { Style::default().fg(app.theme.list_highlight_fg).bg(app.theme.list_highlight_bg).add_modifier(Modifier::BOLD) } else { Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD) })
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
        .style(Style::default().fg(app.theme.fg).add_modifier(Modifier::BOLD))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.border).add_modifier(Modifier::BOLD))
            .title(Span::styled("Selection / Order", Style::default().add_modifier(Modifier::BOLD))))
        .highlight_style(if app.focus==Focus::Right { Style::default().fg(app.theme.sel_highlight_fg).bg(app.theme.sel_highlight_bg).add_modifier(Modifier::BOLD) } else { Style::default().fg(app.theme.ok).add_modifier(Modifier::BOLD) })
        .highlight_symbol("▶ ");
    let mut sel_state = ratatui::widgets::ListState::default();
    if !app.order.is_empty() { sel_state.select(Some(app.order_selected)); }
    f.render_stateful_widget(sel_list, main[1], &mut sel_state);

    // Status + Help bar (split bottom area into three lines)
    let footer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(chunks[2]);
    let status_text = match app.input_mode { InputMode::None => app.status.clone(), _ => format!("{}: {}", app.status, app.input_buffer) };
    let status = Paragraph::new(status_text).style(Style::default().fg(app.theme.fg).add_modifier(Modifier::BOLD));
    f.render_widget(status, footer[0]);
    let help_basic = Paragraph::new("Quit: q  Cancel: Esc  Focus: Tab  Move: ↑/↓/j/k  Select: Space  Run: Enter")
        .style(Style::default().fg(app.theme.fg).add_modifier(Modifier::BOLD));
    f.render_widget(help_basic, footer[1]);
    let help_adv = Paragraph::new("Reorder: u/d/U/D  Rescan: r  Pages: p  Force: F  Options: Depth/Range/Overwrite/Follow")
        .style(Style::default().fg(app.theme.fg).add_modifier(Modifier::BOLD));
    f.render_widget(help_adv, footer[2]);

    // Simple overlay box when in input mode
        if app.input_mode != InputMode::None {
        let popup_h = match app.input_mode {
            InputMode::FilesMenu | InputMode::OptionsMenu | InputMode::PickDepth | InputMode::PickOverwrite => 40,
            InputMode::Help => 60,
            _ => 20,
        };
        let area = centered_rect(60, popup_h, f.size());
        match app.input_mode {
            InputMode::PickMode => {
                // 模式选择弹窗：Merge / Split
                let opts = ["Merge", "Split"];
                let items: Vec<ListItem> = opts.iter().enumerate().map(|(i, s)|{
                    let mark = if i==app.mode_pick_index {">"} else {" "};
                    ListItem::new(Line::from(format!("{} {}", mark, s)))
                }).collect();
                let list = List::new(items)
                    .block(Block::default().title("Pick Mode").borders(Borders::ALL))
                    .highlight_style(Style::default().fg(app.theme.list_highlight_fg).bg(app.theme.list_highlight_bg));
                f.render_widget(Clear, area);
                f.render_widget(list, area);
            }
            InputMode::FilesMenu => {
                let opts = ["Input Path", "Output Path"];
                let items: Vec<ListItem> = opts.iter().enumerate().map(|(i, s)|{
                    let mark = if i==app.files_menu_index {">"} else {" "};
                    ListItem::new(Line::from(format!("{} {}", mark, s)))
                }).collect();
                let list = List::new(items)
                    .block(Block::default().title("Files Menu").borders(Borders::ALL))
                    .highlight_style(Style::default().fg(app.theme.list_highlight_fg).bg(app.theme.list_highlight_bg));
                f.render_widget(Clear, area);
                f.render_widget(list, area);
            }
            InputMode::OptionsMenu => {
                let desc_auto = if app.output_auto_follow {"On"} else {"Off"};
                let desc_over = match app.overwrite_policy { OverwritePolicy::Force=>"Force", OverwritePolicy::Suffix=>"Suffix" };
                let opts = [
                    format!("Depth: {}", app.scan_depth.map(|d| d.to_string()).unwrap_or("∞".into())),
                    format!("Output auto-follow: {}", desc_auto),
                    format!("Overwrite: {}", desc_over),
                    format!("Split range: {}", app.split_group),
                    format!("Split suffix: {}", app.split_suffix),
                ];
                let items: Vec<ListItem> = opts.iter().enumerate().map(|(i, s)|{
                    let mark = if i==app.options_menu_index {">"} else {" "};
                    ListItem::new(Line::from(format!("{} {}", mark, s)))
                }).collect();
                let list = List::new(items)
                    .block(Block::default().title("Options").borders(Borders::ALL))
                    .highlight_style(Style::default().fg(app.theme.list_highlight_fg).bg(app.theme.list_highlight_bg));
                f.render_widget(Clear, area);
                f.render_widget(list, area);
            }
            InputMode::PickOverwrite => {
                let opts = ["Force", "Suffix"];
                let items: Vec<ListItem> = opts.iter().enumerate().map(|(i, s)|{
                    let mark = if i==app.overwrite_pick_index {">"} else {" "};
                    ListItem::new(Line::from(format!("{} {}", mark, s)))
                }).collect();
                let list = List::new(items)
                    .block(Block::default().title("Overwrite Policy").borders(Borders::ALL))
                    .highlight_style(Style::default().fg(app.theme.list_highlight_fg).bg(app.theme.list_highlight_bg));
                f.render_widget(Clear, area);
                f.render_widget(list, area);
            }
            InputMode::PickDepth => {
                let opts = ["1", "2", "3", "∞"];
                let items: Vec<ListItem> = opts.iter().enumerate().map(|(i, s)|{
                    let mark = if i==app.depth_pick_index {">"} else {" "};
                    ListItem::new(Line::from(format!("{} {}", mark, s)))
                }).collect();
                let list = List::new(items)
                    .block(Block::default().title("Scan Depth (1-3/∞)").borders(Borders::ALL))
                    .highlight_style(Style::default().fg(app.theme.list_highlight_fg).bg(app.theme.list_highlight_bg));
                f.render_widget(Clear, area);
                f.render_widget(list, area);
            }
            InputMode::ConfirmLarge => {
                let msg = format!("This will create {} files. Proceed? (y/N)", app.pend_expected);
                let p = Paragraph::new(msg)
                    .block(Block::default().title("Confirm").borders(Borders::ALL))
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(app.theme.fg).add_modifier(Modifier::BOLD));
                f.render_widget(Clear, area);
                f.render_widget(p, area);
            }
            _ => {
                // 输入态：显示可编辑文本并插入可见光标符号
                let (title, show_cursor) = match app.input_mode {
                    InputMode::EditInput => ("Input Path", true),
                    InputMode::EditOutput => ("Output Path", true),
                    InputMode::EditPages => ("Page Ranges", true),
                    InputMode::EditSplitSuffix => ("Split Suffix", true),
                    InputMode::EditSplitRange => ("Split Range (pages per file)", true),
                    _ => ("", false),
                };
                if show_cursor {
                    let (left, right) = app.input_buffer.split_at(app.input_cursor.min(app.input_buffer.len()));
                    let line = Line::from(vec![
                        Span::raw(left.to_string()),
                        Span::styled("▏", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
                        Span::raw(right.to_string()),
                    ]);
                    let p = Paragraph::new(line)
                        .block(Block::default().title(title).borders(Borders::ALL))
                        .wrap(ratatui::widgets::Wrap{ trim: false });
                    f.render_widget(Clear, area);
                    f.render_widget(p, area);
                } else {
                    if matches!(app.input_mode, InputMode::Help) {
                        let help_text = "pdf-ops · Keyboard-only\n\
Mode\n\
- Files: set Input/Output paths\n\
- Mode: Merge / Split\n\
- Options: Depth (1/2/3/∞), Split range (pages per file), Overwrite (Force/Suffix), Output auto-follow\n\
Controls\n\
- Toggle top/menu focus: g\n\
- Navigate: Tab / ← →, ↑/↓/j/k\n\
- Select/Run: Space / Enter\n\
- Cancel: Esc   Quit: q\n\
Notes\n\
- Split: if estimated outputs > 20, confirmation is required.\n\
- Suffix strategy avoids overwriting by appending _1/_2/...\n\
- Paths: supports spaces, quotes, and ~ expansion.";
                        let p = Paragraph::new(help_text)
                            .block(Block::default().title("Help").borders(Borders::ALL))
                            .wrap(ratatui::widgets::Wrap{ trim: true });
                        f.render_widget(Clear, area);
                        f.render_widget(p, area);
                    } else {
                        let p = Paragraph::new(app.input_buffer.clone())
                            .block(Block::default().title(title).borders(Borders::ALL))
                            .wrap(ratatui::widgets::Wrap{ trim: false });
                        f.render_widget(Clear, area);
                        f.render_widget(p, area);
                    }
                }
            }
        }
    }
}

fn ensure_unique_path(p: &PathBuf) -> PathBuf {
    if !p.exists() { return p.clone(); }
    let parent = p.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    for i in 1..10000 {
        let mut name = format!("{}_{i}", stem);
        if !ext.is_empty() { name.push('.'); name.push_str(ext); }
        let cand = parent.join(name);
        if !cand.exists() { return cand; }
    }
    p.clone()
}

fn spawn_merge_job(app: &mut AppState, tx: mpsc::Sender<UiMsg>) {
    app.job_running = true;
    let files: Vec<PathBuf> = app.order.iter().filter_map(|&i| app.files.get(i)).map(|it| it.path.clone()).collect();
    let output = if app.output.is_relative() { app.input_dir.join(&app.output) } else { app.output.clone() };
    let final_output = match app.overwrite_policy {
        OverwritePolicy::Force => output.clone(),
        OverwritePolicy::Suffix => ensure_unique_path(&output),
    };
    let force = matches!(app.overwrite_policy, OverwritePolicy::Force) || app.force;
    let pages = app.pages.clone();
    let tx2 = tx.clone();
    thread::spawn(move || {
        let prog = TuiProgress::new(tx2.clone());
        let res = crate::merge::run_with_files(&files, &final_output, pages.as_deref(), force, &prog);
        let note = format!("{}", final_output.display());
        let _ = tx2.send(UiMsg::JobDone(res, note));
    });
}

fn choose_out_dir(input_dir: &PathBuf, output: &PathBuf) -> PathBuf {
    let out = if output.is_relative() { input_dir.join(output) } else { output.clone() };
    // if looks like a pdf file, use its parent
    if out.extension().map(|e| e.eq_ignore_ascii_case("pdf")).unwrap_or(false) {
        if let Some(p) = out.parent() { return p.to_path_buf(); }
    }
    out
}

// split job handled via spawn_split_job_params after preflight

fn make_ranges_spec(total: usize, group: usize) -> String {
    if total==0 || group==0 { return String::new(); }
    let mut parts = Vec::new();
    let mut start = 1usize;
    while start <= total {
        let end = (start + group - 1).min(total);
        parts.push(format!("{}-{}", start, end));
        start = end + 1;
    }
    parts.join(",")
}

fn spawn_split_job_params(input: PathBuf, out_dir: PathBuf, each: bool, ranges: Option<String>, pattern: String, force: bool, tx: mpsc::Sender<UiMsg>) {
    let tx2 = tx.clone();
    thread::spawn(move || {
        let prog = TuiProgress::new(tx2.clone());
        let res = crate::split::run(&input, &out_dir, each, ranges.as_deref(), &pattern, force, &prog);
        let note = format!("{} -> {}", input.display(), out_dir.display());
        let _ = tx2.send(UiMsg::JobDone(res, note));
    });
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
