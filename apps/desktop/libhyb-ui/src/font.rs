//! Embedded Bitmap Font Engine (8x16 ASCII & Symbols)

pub const FONT_WIDTH: usize = 8;
pub const FONT_HEIGHT: usize = 16;

/// Procedural ASCII 8x16 glyph generator
pub fn get_glyph(ch: u8) -> [u8; 16] {
    let mut rows = [0u8; 16];
    match ch {
        b' ' => {}
        b'!' => {
            for r in 2..10 { rows[r] = 0x18; }
            rows[12] = 0x18;
            rows[13] = 0x18;
        }
        b'"' => {
            rows[2] = 0x66; rows[3] = 0x66; rows[4] = 0x66;
        }
        b'#' => {
            rows[4] = 0x24; rows[5] = 0x7E; rows[6] = 0x24;
            rows[7] = 0x24; rows[8] = 0x7E; rows[9] = 0x24;
        }
        b'$' => {
            rows[2] = 0x18; rows[3] = 0x3C; rows[4] = 0x58;
            rows[5] = 0x38; rows[6] = 0x1C; rows[7] = 0x1A;
            rows[8] = 0x3C; rows[9] = 0x18;
        }
        b'%' => {
            rows[3] = 0x62; rows[4] = 0x64; rows[5] = 0x08;
            rows[6] = 0x10; rows[7] = 0x26; rows[8] = 0x46;
        }
        b'&' => {
            rows[3] = 0x30; rows[4] = 0x48; rows[5] = 0x30;
            rows[6] = 0x4A; rows[7] = 0x44; rows[8] = 0x3A;
        }
        b'\'' => {
            rows[2] = 0x18; rows[3] = 0x18; rows[4] = 0x10;
        }
        b'(' => {
            rows[2] = 0x0C; rows[3] = 0x18; rows[4] = 0x30;
            rows[5] = 0x30; rows[6] = 0x30; rows[7] = 0x18; rows[8] = 0x0C;
        }
        b')' => {
            rows[2] = 0x30; rows[3] = 0x18; rows[4] = 0x0C;
            rows[5] = 0x0C; rows[6] = 0x0C; rows[7] = 0x18; rows[8] = 0x30;
        }
        b'*' => {
            rows[3] = 0x00; rows[4] = 0x66; rows[5] = 0x3C;
            rows[6] = 0x66; rows[7] = 0x00;
        }
        b'+' => {
            rows[5] = 0x18; rows[6] = 0x18; rows[7] = 0x7E;
            rows[8] = 0x18; rows[9] = 0x18;
        }
        b',' => {
            rows[11] = 0x18; rows[12] = 0x18; rows[13] = 0x10;
        }
        b'-' => {
            rows[7] = 0x7E; rows[8] = 0x7E;
        }
        b'.' => {
            rows[12] = 0x18; rows[13] = 0x18;
        }
        b'/' => {
            for r in 2..13 { rows[r] = 0x80 >> (r - 2) * 8 / 11; }
        }
        b'0'..=b'9' => {
            let digit = ch - b'0';
            match digit {
                0 => {
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x6E;
                    rows[6] = 0x76; rows[7] = 0x66; rows[8] = 0x3C;
                }
                1 => {
                    rows[3] = 0x18; rows[4] = 0x38; rows[5] = 0x18;
                    rows[6] = 0x18; rows[7] = 0x18; rows[8] = 0x7E;
                }
                2 => {
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x06;
                    rows[6] = 0x1C; rows[7] = 0x30; rows[8] = 0x7E;
                }
                3 => {
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x1C;
                    rows[6] = 0x06; rows[7] = 0x66; rows[8] = 0x3C;
                }
                4 => {
                    rows[3] = 0x44; rows[4] = 0x64; rows[5] = 0x54;
                    rows[6] = 0x7E; rows[7] = 0x04; rows[8] = 0x04;
                }
                5 => {
                    rows[3] = 0x7E; rows[4] = 0x40; rows[5] = 0x7C;
                    rows[6] = 0x06; rows[7] = 0x46; rows[8] = 0x3C;
                }
                6 => {
                    rows[3] = 0x3C; rows[4] = 0x60; rows[5] = 0x7C;
                    rows[6] = 0x66; rows[7] = 0x66; rows[8] = 0x3C;
                }
                7 => {
                    rows[3] = 0x7E; rows[4] = 0x06; rows[5] = 0x0C;
                    rows[6] = 0x18; rows[7] = 0x18; rows[8] = 0x18;
                }
                8 => {
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x3C;
                    rows[6] = 0x66; rows[7] = 0x66; rows[8] = 0x3C;
                }
                9 => {
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x3E;
                    rows[6] = 0x06; rows[7] = 0x06; rows[8] = 0x3C;
                }
                _ => {}
            }
        }
        b':' => {
            rows[6] = 0x18; rows[7] = 0x18;
            rows[10] = 0x18; rows[11] = 0x18;
        }
        b';' => {
            rows[6] = 0x18; rows[7] = 0x18;
            rows[10] = 0x18; rows[11] = 0x18; rows[12] = 0x10;
        }
        b'<' => {
            rows[5] = 0x0C; rows[6] = 0x18; rows[7] = 0x30;
            rows[8] = 0x18; rows[9] = 0x0C;
        }
        b'=' => {
            rows[6] = 0x7E; rows[8] = 0x7E;
        }
        b'>' => {
            rows[5] = 0x30; rows[6] = 0x18; rows[7] = 0x0C;
            rows[8] = 0x18; rows[9] = 0x30;
        }
        b'?' => {
            rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x0C;
            rows[6] = 0x18; rows[7] = 0x18; rows[9] = 0x18;
        }
        b'@' => {
            rows[3] = 0x3C; rows[4] = 0x42; rows[5] = 0x9D;
            rows[6] = 0xA5; rows[7] = 0x42; rows[8] = 0x3C;
        }
        b'A'..=b'Z' => {
            let offset = ch - b'A';
            match offset {
                0 => { // A
                    rows[3] = 0x18; rows[4] = 0x3C; rows[5] = 0x66;
                    rows[6] = 0x7E; rows[7] = 0x66; rows[8] = 0x66;
                }
                1 => { // B
                    rows[3] = 0x7C; rows[4] = 0x66; rows[5] = 0x7C;
                    rows[6] = 0x66; rows[7] = 0x66; rows[8] = 0x7C;
                }
                2 => { // C
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x60;
                    rows[6] = 0x60; rows[7] = 0x66; rows[8] = 0x3C;
                }
                3 => { // D
                    rows[3] = 0x78; rows[4] = 0x6C; rows[5] = 0x66;
                    rows[6] = 0x66; rows[7] = 0x6C; rows[8] = 0x78;
                }
                4 => { // E
                    rows[3] = 0x7E; rows[4] = 0x60; rows[5] = 0x7C;
                    rows[6] = 0x60; rows[7] = 0x60; rows[8] = 0x7E;
                }
                5 => { // F
                    rows[3] = 0x7E; rows[4] = 0x60; rows[5] = 0x7C;
                    rows[6] = 0x60; rows[7] = 0x60; rows[8] = 0x60;
                }
                6 => { // G
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x60;
                    rows[6] = 0x6E; rows[7] = 0x66; rows[8] = 0x3A;
                }
                7 => { // H
                    rows[3] = 0x66; rows[4] = 0x66; rows[5] = 0x7E;
                    rows[6] = 0x66; rows[7] = 0x66; rows[8] = 0x66;
                }
                8 => { // I
                    rows[3] = 0x3C; rows[4] = 0x18; rows[5] = 0x18;
                    rows[6] = 0x18; rows[7] = 0x18; rows[8] = 0x3C;
                }
                11 => { // L
                    rows[3] = 0x60; rows[4] = 0x60; rows[5] = 0x60;
                    rows[6] = 0x60; rows[7] = 0x60; rows[8] = 0x7E;
                }
                13 => { // N
                    rows[3] = 0x66; rows[4] = 0x76; rows[5] = 0x7E;
                    rows[6] = 0x6E; rows[7] = 0x66; rows[8] = 0x66;
                }
                14 => { // O
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x66;
                    rows[6] = 0x66; rows[7] = 0x66; rows[8] = 0x3C;
                }
                15 => { // P
                    rows[3] = 0x7C; rows[4] = 0x66; rows[5] = 0x7C;
                    rows[6] = 0x60; rows[7] = 0x60; rows[8] = 0x60;
                }
                17 => { // R
                    rows[3] = 0x7C; rows[4] = 0x66; rows[5] = 0x7C;
                    rows[6] = 0x6C; rows[7] = 0x66; rows[8] = 0x66;
                }
                18 => { // S
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x38;
                    rows[6] = 0x0E; rows[7] = 0x66; rows[8] = 0x3C;
                }
                19 => { // T
                    rows[3] = 0x7E; rows[4] = 0x18; rows[5] = 0x18;
                    rows[6] = 0x18; rows[7] = 0x18; rows[8] = 0x18;
                }
                21 => { // V
                    rows[3] = 0x66; rows[4] = 0x66; rows[5] = 0x66;
                    rows[6] = 0x66; rows[7] = 0x3C; rows[8] = 0x18;
                }
                22 => { // W
                    rows[3] = 0x63; rows[4] = 0x63; rows[5] = 0x6B;
                    rows[6] = 0x7F; rows[7] = 0x77; rows[8] = 0x63;
                }
                23 => { // X
                    rows[3] = 0x66; rows[4] = 0x3C; rows[5] = 0x18;
                    rows[6] = 0x3C; rows[7] = 0x66; rows[8] = 0x66;
                }
                _ => {
                    rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x66;
                    rows[6] = 0x66; rows[7] = 0x66; rows[8] = 0x3C;
                }
            }
        }
        b'a'..=b'z' => {
            let offset = ch - b'a';
            match offset {
                0 => { // a
                    rows[5] = 0x3C; rows[6] = 0x06; rows[7] = 0x3E;
                    rows[8] = 0x66; rows[9] = 0x3B;
                }
                1 => { // b
                    rows[3] = 0x60; rows[4] = 0x60; rows[5] = 0x7C;
                    rows[6] = 0x66; rows[7] = 0x66; rows[8] = 0x7C;
                }
                2 => { // c
                    rows[5] = 0x3C; rows[6] = 0x66; rows[7] = 0x60;
                    rows[8] = 0x66; rows[9] = 0x3C;
                }
                3 => { // d
                    rows[3] = 0x06; rows[4] = 0x06; rows[5] = 0x3E;
                    rows[6] = 0x66; rows[7] = 0x66; rows[8] = 0x3E;
                }
                4 => { // e
                    rows[5] = 0x3C; rows[6] = 0x66; rows[7] = 0x7E;
                    rows[8] = 0x60; rows[9] = 0x3C;
                }
                8 => { // i
                    rows[3] = 0x18; rows[5] = 0x18; rows[6] = 0x18;
                    rows[7] = 0x18; rows[8] = 0x18; rows[9] = 0x3C;
                }
                11 => { // l
                    rows[3] = 0x38; rows[4] = 0x18; rows[5] = 0x18;
                    rows[6] = 0x18; rows[7] = 0x18; rows[8] = 0x3C;
                }
                14 => { // o
                    rows[5] = 0x3C; rows[6] = 0x66; rows[7] = 0x66;
                    rows[8] = 0x66; rows[9] = 0x3C;
                }
                17 => { // r
                    rows[5] = 0x6C; rows[6] = 0x76; rows[7] = 0x60;
                    rows[8] = 0x60; rows[9] = 0x60;
                }
                18 => { // s
                    rows[5] = 0x3E; rows[6] = 0x60; rows[7] = 0x3C;
                    rows[8] = 0x06; rows[9] = 0x7C;
                }
                19 => { // t
                    rows[3] = 0x18; rows[4] = 0x7E; rows[5] = 0x18;
                    rows[6] = 0x18; rows[7] = 0x18; rows[8] = 0x1C;
                }
                _ => {
                    rows[5] = 0x3C; rows[6] = 0x66; rows[7] = 0x66;
                    rows[8] = 0x66; rows[9] = 0x3C;
                }
            }
        }
        b'[' => {
            rows[2] = 0x3C; rows[3] = 0x30; rows[4] = 0x30;
            rows[5] = 0x30; rows[6] = 0x30; rows[7] = 0x30; rows[8] = 0x3C;
        }
        b']' => {
            rows[2] = 0x3C; rows[3] = 0x0C; rows[4] = 0x0C;
            rows[5] = 0x0C; rows[6] = 0x0C; rows[7] = 0x0C; rows[8] = 0x3C;
        }
        b'_' => {
            rows[14] = 0xFF; rows[15] = 0xFF;
        }
        _ => {
            rows[3] = 0x3C; rows[4] = 0x66; rows[5] = 0x66;
            rows[6] = 0x66; rows[7] = 0x66; rows[8] = 0x3C;
        }
    }
    rows
}
