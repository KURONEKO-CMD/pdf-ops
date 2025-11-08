#![cfg(feature = "tui")]

use ratatui::style::Color;

#[derive(Clone, Debug)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub accent: Color,
    pub list_highlight_bg: Color,
    pub list_highlight_fg: Color,
    pub sel_highlight_bg: Color,
    pub sel_highlight_fg: Color,
    pub ok: Color,
}

impl Theme {
    pub fn gitui_dark() -> Self {
        // High-contrast yet low-glare variant
        Self {
            bg: Color::Rgb(8, 8, 8),
            fg: Color::Rgb(230, 230, 230),
            border: Color::Rgb(120, 120, 120),
            accent: Color::Cyan,
            list_highlight_bg: Color::Blue,
            list_highlight_fg: Color::White,
            sel_highlight_bg: Color::Green,
            sel_highlight_fg: Color::Black,
            ok: Color::Green,
        }
    }

    pub fn light() -> Self {
        Self {
            bg: Color::Rgb(250, 250, 250),
            fg: Color::Rgb(30, 30, 30),
            border: Color::Rgb(200, 200, 200),
            accent: Color::Rgb(25, 118, 210),
            list_highlight_bg: Color::Rgb(187, 222, 251),
            list_highlight_fg: Color::Rgb(0, 0, 0),
            sel_highlight_bg: Color::Rgb(200, 230, 201),
            sel_highlight_fg: Color::Rgb(0, 0, 0),
            ok: Color::Rgb(46, 160, 67),
        }
    }
}

#[allow(dead_code)]
pub fn resolve(name: Option<String>) -> Theme {
    match name.as_deref() {
        Some("light") => Theme::light(),
        _ => Theme::gitui_dark(),
    }
}
