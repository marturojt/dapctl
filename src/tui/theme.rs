//! Colour palette. Default: green phosphor on black.

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
