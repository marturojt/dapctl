//! Colour palette. Default: green phosphor on black.

pub struct Theme {
    pub fg: (u8, u8, u8),
    pub bg: (u8, u8, u8),
    pub warn: (u8, u8, u8),
    pub err: (u8, u8, u8),
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            fg: (0x8a, 0xff, 0x80),
            bg: (0x00, 0x00, 0x00),
            warn: (0xff, 0xcc, 0x00),
            err: (0xff, 0x5f, 0x5f),
        }
    }
}
