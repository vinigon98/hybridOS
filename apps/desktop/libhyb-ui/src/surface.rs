//! Framebuffer Surface and Software Rasterizer with Native PNG Export

use crate::font::{get_glyph, FONT_HEIGHT, FONT_WIDTH};
use crate::geometry::Rect;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// 32-bit BGRA/RGBA Compositor Framebuffer Surface
#[derive(Debug, Clone)]
pub struct Surface {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u32>,
    pub clip_rect: Rect,
}

impl Surface {
    /// Create new surface initialized to given background color
    pub fn new(width: usize, height: usize, bg_color: u32) -> Self {
        Self {
            width,
            height,
            buffer: vec![bg_color; width * height],
            clip_rect: Rect::new(0, 0, width as u32, height as u32),
        }
    }

    /// Set clipping boundary for drawing operations
    pub fn set_clip(&mut self, rect: Rect) {
        self.clip_rect = rect;
    }

    /// Reset clipping to entire surface
    pub fn reset_clip(&mut self) {
        self.clip_rect = Rect::new(0, 0, self.width as u32, self.height as u32);
    }

    /// Draw single pixel with clipping
    #[inline(always)]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        let ix = x as i32;
        let iy = y as i32;
        if ix >= self.clip_rect.left()
            && ix < self.clip_rect.right()
            && iy >= self.clip_rect.top()
            && iy < self.clip_rect.bottom()
            && x < self.width
            && y < self.height
        {
            self.buffer[y * self.width + x] = color;
        }
    }

    /// Get pixel at coordinate
    #[inline(always)]
    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.buffer[y * self.width + x]
        } else {
            0
        }
    }

    /// Fill solid rectangle with bounds checking
    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: u32) {
        let start_x = (x.max(self.clip_rect.left())).max(0) as usize;
        let end_x = (x.saturating_add(w as i32).min(self.clip_rect.right())).max(0) as usize;
        let start_y = (y.max(self.clip_rect.top())).max(0) as usize;
        let end_y = (y.saturating_add(h as i32).min(self.clip_rect.bottom())).max(0) as usize;

        let end_x = end_x.min(self.width);
        let end_y = end_y.min(self.height);

        for py in start_y..end_y {
            let offset = py * self.width;
            for px in start_x..end_x {
                self.buffer[offset + px] = color;
            }
        }
    }

    /// Draw 1px rectangle outline
    pub fn draw_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.fill_rect(x, y, w, 1, color);
        self.fill_rect(x, y + (h as i32) - 1, w, 1, color);
        self.fill_rect(x, y, 1, h, color);
        self.fill_rect(x + (w as i32) - 1, y, 1, h, color);
    }

    /// Draw straight line using Bresenham algorithm
    pub fn draw_line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: u32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x0 >= 0 && y0 >= 0 {
                self.set_pixel(x0 as usize, y0 as usize, color);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Draw simulated drop shadow
    pub fn draw_drop_shadow(&mut self, x: i32, y: i32, w: u32, h: u32, offset: i32) {
        self.fill_rect(x + offset, y + offset, w, h, 0x44000000);
    }

    /// Draw character glyph with scaling factor
    pub fn draw_char(&mut self, x: usize, y: usize, ch: u8, color: u32) {
        let glyph = get_glyph(ch);
        for row in 0..FONT_HEIGHT {
            let py = y + row;
            if py >= self.height {
                break;
            }
            let bits = glyph[row];
            for col in 0..FONT_WIDTH {
                let px = x + col;
                if px >= self.width {
                    break;
                }
                if (bits & (0x80 >> col)) != 0 {
                    self.set_pixel(px, py, color);
                }
            }
        }
    }

    /// Draw string text
    pub fn draw_string(&mut self, mut x: usize, y: usize, text: &str, color: u32) {
        for &byte in text.as_bytes() {
            if x + FONT_WIDTH > self.width {
                break;
            }
            self.draw_char(x, y, byte, color);
            x += FONT_WIDTH;
        }
    }

    /// Blit a region from another surface
    pub fn blit(
        &mut self,
        src: &Surface,
        src_x: usize,
        src_y: usize,
        dst_x: usize,
        dst_y: usize,
        w: usize,
        h: usize,
    ) {
        let copy_w = w.min(src.width.saturating_sub(src_x)).min(self.width.saturating_sub(dst_x));
        let copy_h = h.min(src.height.saturating_sub(src_y)).min(self.height.saturating_sub(dst_y));

        for row in 0..copy_h {
            let src_offset = (src_y + row) * src.width + src_x;
            let dst_offset = (dst_y + row) * self.width + dst_x;
            self.buffer[dst_offset..dst_offset + copy_w]
                .copy_from_slice(&src.buffer[src_offset..src_offset + copy_w]);
        }
    }

    /// Export surface contents to a standard uncompressed PNG file
    pub fn export_png(&self, path: &Path) -> Result<(), String> {
        let mut file = File::create(path).map_err(|e| e.to_string())?;

        // 1. Signature
        file.write_all(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
            .map_err(|e| e.to_string())?;

        // 2. IHDR Chunk
        let mut ihdr_data = Vec::with_capacity(13);
        ihdr_data.extend_from_slice(&(self.width as u32).to_be_bytes());
        ihdr_data.extend_from_slice(&(self.height as u32).to_be_bytes());
        ihdr_data.push(8); // Bit depth: 8 bits per channel
        ihdr_data.push(6); // Color type: 6 = RGBA
        ihdr_data.push(0); // Compression method: deflate
        ihdr_data.push(0); // Filter method: standard
        ihdr_data.push(0); // Interlace method: none
        write_chunk(&mut file, b"IHDR", &ihdr_data)?;

        // 3. IDAT Chunk (Raw RGBA scanlines with filter byte 0)
        let mut raw_scanlines = Vec::with_capacity(self.height * (self.width * 4 + 1));
        for y in 0..self.height {
            raw_scanlines.push(0); // Filter type 0 (None)
            for x in 0..self.width {
                let pixel = self.buffer[y * self.width + x];
                let a = ((pixel >> 24) & 0xFF) as u8;
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;
                raw_scanlines.push(r);
                raw_scanlines.push(g);
                raw_scanlines.push(b);
                raw_scanlines.push(a);
            }
        }

        // Deflate stored/uncompressed blocks
        let deflated = deflate_stored(&raw_scanlines);
        write_chunk(&mut file, b"IDAT", &deflated)?;

        // 4. IEND Chunk
        write_chunk(&mut file, b"IEND", &[])?;

        file.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Write PNG chunk with length, type, payload, and CRC32
fn write_chunk(file: &mut File, chunk_type: &[u8; 4], data: &[u8]) -> Result<(), String> {
    file.write_all(&(data.len() as u32).to_be_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(chunk_type).map_err(|e| e.to_string())?;
    if !data.is_empty() {
        file.write_all(data).map_err(|e| e.to_string())?;
    }

    let mut crc_val = crc32_init();
    crc_val = crc32_update(crc_val, chunk_type);
    crc_val = crc32_update(crc_val, data);
    let final_crc = crc32_finalize(crc_val);

    file.write_all(&final_crc.to_be_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Encapsulate raw byte stream in valid zlib deflate stored blocks with Adler-32
fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // Zlib header: CMF = 0x78 (deflate, 32K window), FLG = 0x01 (check bits)
    out.push(0x78);
    out.push(0x01);

    const CHUNK_SIZE: usize = 65535;
    let mut offset = 0;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let block_len = remaining.min(CHUNK_SIZE);
        let is_last = (offset + block_len) == data.len();
        let bfinal = if is_last { 0x01 } else { 0x00 };
        out.push(bfinal); // Stored block (BTYPE=00)

        let len16 = block_len as u16;
        let nlen16 = !len16;
        out.extend_from_slice(&len16.to_le_bytes());
        out.extend_from_slice(&nlen16.to_le_bytes());
        out.extend_from_slice(&data[offset..offset + block_len]);

        offset += block_len;
    }

    // Adler-32 checksum
    let adler = adler32(data);
    out.extend_from_slice(&adler.to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in data {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

fn crc32_init() -> u32 {
    0xFFFFFFFF
}

fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

fn crc32_finalize(crc: u32) -> u32 {
    crc ^ 0xFFFFFFFF
}
