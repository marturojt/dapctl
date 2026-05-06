//! Colour palette. Default: green phosphor on black.
//! Respects NO_COLOR (https://no-color.org): when set, all colours collapse
//! to terminal defaults so the output is rendered in monochrome.

use ratatui::style::Color;

pub struct Theme {
    pub fg: Color,
    pub bg: Color,
    pub warn: Color,
    pub err: Color,
    pub muted: Color,
    pub sel_fg: Color,
    pub sel_bg: Color,
}

impl Theme {
    /// Construct the active theme, honouring the `NO_COLOR` environment variable.
    pub fn new() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            Self::no_color()
        } else {
            Self::default()
        }
    }

    fn no_color() -> Self {
        Self {
            fg: Color::Reset,
            bg: Color::Reset,
            warn: Color::Reset,
            err: Color::Reset,
            muted: Color::Reset,
            sel_fg: Color::Black,
            sel_bg: Color::White,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            fg: Color::Rgb(0x8a, 0xff, 0x80),
            bg: Color::Rgb(0x00, 0x00, 0x00),
            warn: Color::Rgb(0xff, 0xcc, 0x00),
            err: Color::Rgb(0xff, 0x5f, 0x5f),
            muted: Color::Rgb(0x4a, 0x7a, 0x45),
            sel_fg: Color::Rgb(0x00, 0x00, 0x00),
            sel_bg: Color::Rgb(0x8a, 0xff, 0x80),
        }
    }
}
