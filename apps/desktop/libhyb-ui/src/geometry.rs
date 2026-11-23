//! 2D Geometry and Color primitives for libhyb-ui

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn left(&self) -> i32 {
        self.x
    }

    pub fn top(&self) -> i32 {
        self.y
    }

    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.left() && p.x < self.right() && p.y >= self.top() && p.y < self.bottom()
    }

    pub fn contains_xy(&self, px: i32, py: i32) -> bool {
        self.contains(Point::new(px, py))
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    pub fn from_points(p1: Point, p2: Point) -> Self {
        let min_x = p1.x.min(p2.x);
        let min_y = p1.y.min(p2.y);
        let max_x = p1.x.max(p2.x);
        let max_y = p1.y.max(p2.y);
        Self {
            x: min_x,
            y: min_y,
            width: (max_x - min_x) as u32,
            height: (max_y - min_y) as u32,
        }
    }
}

pub mod colors {
    pub const COLOR_BG: u32 = 0xFF181825;
    pub const COLOR_SURFACE: u32 = 0xFF202020;
    pub const COLOR_CARD: u32 = 0xFF2B2B2B;
    pub const COLOR_TITLEBAR: u32 = 0xFF1F1F1F;
    pub const COLOR_ACCENT: u32 = 0xFF0078D4;
    pub const COLOR_BORDER: u32 = 0xFF444444;
    pub const COLOR_BORDER_ACTIVE: u32 = 0xFF666666;
    pub const COLOR_TEXT: u32 = 0xFFFFFFFF;
    pub const COLOR_TEXT_MUTED: u32 = 0xFFAAAAAA;
    pub const COLOR_SUCCESS: u32 = 0xFF50FA7B;
    pub const COLOR_CYAN: u32 = 0xFF8BE9FD;
    pub const COLOR_DANGER: u32 = 0xFFE81123;
    pub const COLOR_SELECTION: u32 = 0x660078D4;
    pub const COLOR_HOVER: u32 = 0x22FFFFFF;
}
