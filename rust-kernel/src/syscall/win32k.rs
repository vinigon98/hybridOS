//! Win32k Graphical Subsystem & Shadow SSDT in Rust.
//!
//! Implements the Windows NT GUI subsystem (Win32k.sys) covering:
//! - Win32k Shadow SSDT Table (Service numbers 0x1000..0x1200)
//! - Window Manager (User32 in kernel):
//!   - NtUserCreateWindowEx (0x1000): window class, title, styles, coordinates, returns HWND
//!   - NtUserDestroyWindow (0x1001): destroys window and child tree hierarchy
//!   - NtUserShowWindow (0x1002): handles visibility and state transitions
//!   - NtUserSetWindowPos (0x1003): coordinates, dimensions, and Z-order management
//!   - Thread Message Queue: MSG structure, WM_* messages
//!   - Message handling: NtUserGetMessage (0x1004), NtUserPeekMessage (0x1005),
//!     NtUserPostMessage (0x1006), NtUserDispatchMessage (0x1007)
//!   - Global Window Tree: DesktopWindow, parent/child hierarchy, Z-order list, hit testing
//! - GDI (Graphics Device Interface in kernel):
//!   - Device Contexts: NtGdiCreateCompatibleDC (0x1010), NtGdiDeleteDC (0x1011)
//!   - Bitmaps: NtGdiCreateBitmap (0x1012), NtGdiCreateCompatibleBitmap (0x1013),
//!     NtGdiSelectBitmap (0x1014)
//!   - Drawing: NtGdiBitBlt (0x1015, SRCCOPY and raster ops),
//!     NtGdiExtTextOutW (0x1016), NtGdiRectangle (0x1017)
//!   - Extended GDI primitives: NtGdiSetPixel (0x1018), NtGdiGetPixel (0x1019),
//!     NtGdiCreateSolidBrush (0x101A), NtGdiSelectBrush (0x101B), NtGdiFillRect (0x101C),
//!     NtGdiLineTo (0x101D), NtUserInvalidateRect (0x101E)
//! - SSDT Dispatch Table: `dispatch_win32k_syscall(proc, service_id, args...)`

#[allow(unused_imports)]
use alloc::string::{String, ToString};
#[allow(unused_imports)]
use alloc::vec::Vec;
#[allow(unused_imports)]
use alloc::boxed::Box;
#[allow(unused_imports)]
use crate::compat::FloatExt;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};

use crate::process::HybridProcess;
use crate::types::NtStatus;

// ============================================================================
// Win32k Shadow SSDT Service Numbers (0x1000..0x1200)
// ============================================================================

// Window Manager (User32 in kernel)
pub const SYS_NT_USER_CREATE_WINDOW_EX: u64      = 0x1000;
pub const SYS_NT_USER_DESTROY_WINDOW: u64        = 0x1001;
pub const SYS_NT_USER_SHOW_WINDOW: u64           = 0x1002;
pub const SYS_NT_USER_SET_WINDOW_POS: u64        = 0x1003;
pub const SYS_NT_USER_GET_MESSAGE: u64           = 0x1004;
pub const SYS_NT_USER_PEEK_MESSAGE: u64          = 0x1005;
pub const SYS_NT_USER_POST_MESSAGE: u64          = 0x1006;
pub const SYS_NT_USER_DISPATCH_MESSAGE: u64      = 0x1007;
pub const SYS_NT_USER_GET_DC: u64                = 0x1008;
pub const SYS_NT_USER_RELEASE_DC: u64            = 0x1009;
pub const SYS_NT_USER_GET_CLIENT_RECT: u64       = 0x100A;
pub const SYS_NT_USER_GET_WINDOW_RECT: u64       = 0x100B;
pub const SYS_NT_USER_FIND_WINDOW_EX: u64        = 0x100C;
pub const SYS_NT_USER_DEF_WINDOW_PROC: u64       = 0x100D;
pub const SYS_NT_USER_UPDATE_WINDOW: u64         = 0x100E;
pub const SYS_NT_USER_GET_DESKTOP_WINDOW: u64    = 0x100F;

// GDI (Graphics Device Interface in kernel)
pub const SYS_NT_GDI_CREATE_COMPATIBLE_DC: u64   = 0x1010;
pub const SYS_NT_GDI_DELETE_DC: u64              = 0x1011;
pub const SYS_NT_GDI_CREATE_BITMAP: u64          = 0x1012;
pub const SYS_NT_GDI_CREATE_COMPATIBLE_BITMAP: u64 = 0x1013;
pub const SYS_NT_GDI_SELECT_BITMAP: u64          = 0x1014;
pub const SYS_NT_GDI_BIT_BLT: u64                = 0x1015;
pub const SYS_NT_GDI_EXT_TEXT_OUT_W: u64         = 0x1016;
pub const SYS_NT_GDI_RECTANGLE: u64              = 0x1017;
pub const SYS_NT_GDI_SET_PIXEL: u64              = 0x1018;
pub const SYS_NT_GDI_GET_PIXEL: u64              = 0x1019;
pub const SYS_NT_GDI_CREATE_SOLID_BRUSH: u64     = 0x101A;
pub const SYS_NT_GDI_SELECT_BRUSH: u64           = 0x101B;
pub const SYS_NT_GDI_FILL_RECT: u64              = 0x101C;
pub const SYS_NT_GDI_LINE_TO: u64                = 0x101D;
pub const SYS_NT_USER_INVALIDATE_RECT: u64       = 0x101E;

pub const WIN32K_SSDT_MIN: u64                   = 0x1000;
pub const WIN32K_SSDT_MAX: u64                   = 0x1200;

// ============================================================================
// Win32 Window Styles & Extended Styles
// ============================================================================

pub const WS_OVERLAPPED: u32       = 0x00000000;
pub const WS_POPUP: u32            = 0x80000000;
pub const WS_CHILD: u32            = 0x40000000;
pub const WS_MINIMIZE: u32         = 0x20000000;
pub const WS_VISIBLE: u32          = 0x10000000;
pub const WS_DISABLED: u32         = 0x08000000;
pub const WS_CLIPSIBLINGS: u32     = 0x04000000;
pub const WS_CLIPCHILDREN: u32     = 0x02000000;
pub const WS_MAXIMIZE: u32         = 0x01000000;
pub const WS_CAPTION: u32          = 0x00C00000;
pub const WS_BORDER: u32           = 0x00800000;
pub const WS_DLGFRAME: u32         = 0x00400000;
pub const WS_VSCROLL: u32          = 0x00200000;
pub const WS_HSCROLL: u32          = 0x00100000;
pub const WS_SYSMENU: u32          = 0x00080000;
pub const WS_THICKFRAME: u32       = 0x00040000;
pub const WS_MINIMIZEBOX: u32      = 0x00020000;
pub const WS_MAXIMIZEBOX: u32      = 0x00010000;
pub const WS_OVERLAPPEDWINDOW: u32 = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
pub const WS_TILEDWINDOW: u32      = WS_OVERLAPPEDWINDOW;

// Extended Window Styles
pub const WS_EX_DLGMODALFRAME: u32 = 0x00000001;
pub const WS_EX_NOPARENTNOTIFY: u32= 0x00000004;
pub const WS_EX_TOPMOST: u32       = 0x00000008;
pub const WS_EX_ACCEPTFILES: u32   = 0x00000010;
pub const WS_EX_TRANSPARENT: u32   = 0x00000020;
pub const WS_EX_MDICHILD: u32      = 0x00000040;
pub const WS_EX_TOOLWINDOW: u32    = 0x00000080;
pub const WS_EX_WINDOWEDGE: u32    = 0x00000100;
pub const WS_EX_CLIENTEDGE: u32    = 0x00000200;
pub const WS_EX_CONTEXTHELP: u32   = 0x00000400;
pub const WS_EX_APPWINDOW: u32     = 0x00040000;
pub const WS_EX_LAYERED: u32       = 0x00080000;

// ShowWindow Commands
pub const SW_HIDE: u32             = 0;
pub const SW_SHOWNORMAL: u32       = 1;
pub const SW_NORMAL: u32           = 1;
pub const SW_SHOWMINIMIZED: u32    = 2;
pub const SW_MAXIMIZE: u32         = 3;
pub const SW_SHOWMAXIMIZED: u32    = 3;
pub const SW_SHOWNOACTIVATE: u32   = 4;
pub const SW_SHOW: u32             = 5;
pub const SW_MINIMIZE: u32         = 6;
pub const SW_SHOWMINNOACTIVE: u32  = 7;
pub const SW_SHOWNA: u32           = 8;
pub const SW_RESTORE: u32          = 9;

// SetWindowPos Flags
pub const SWP_NOSIZE: u32          = 0x0001;
pub const SWP_NOMOVE: u32          = 0x0002;
pub const SWP_NOZORDER: u32        = 0x0004;
pub const SWP_NOREDRAW: u32        = 0x0008;
pub const SWP_NOACTIVATE: u32      = 0x0010;
pub const SWP_FRAMECHANGED: u32    = 0x0020;
pub const SWP_SHOWWINDOW: u32      = 0x0040;
pub const SWP_HIDEWINDOW: u32      = 0x0080;
pub const SWP_NOCOPYBITS: u32      = 0x0100;
pub const SWP_NOOWNERZORDER: u32   = 0x0200;
pub const SWP_NOSENDCHANGING: u32  = 0x0400;

// Special HWND values for SetWindowPos
pub const HWND_TOP: usize          = 0;
pub const HWND_BOTTOM: usize       = 1;
pub const HWND_TOPMOST: usize      = usize::MAX;
pub const HWND_NOTOPMOST: usize    = usize::MAX - 1;

// Default coordinate marker
pub const CW_USEDEFAULT: i32       = 0x80000000u32 as i32;

// ============================================================================
// Windows Messages (WM_*)
// ============================================================================

pub const WM_NULL: u32             = 0x0000;
pub const WM_CREATE: u32           = 0x0001;
pub const WM_DESTROY: u32          = 0x0002;
pub const WM_MOVE: u32             = 0x0003;
pub const WM_SIZE: u32             = 0x0005;
pub const WM_ACTIVATE: u32         = 0x0006;
pub const WM_SETFOCUS: u32         = 0x0007;
pub const WM_KILLFOCUS: u32        = 0x0008;
pub const WM_ENABLE: u32           = 0x000A;
pub const WM_SETREDRAW: u32        = 0x000B;
pub const WM_SETTEXT: u32          = 0x000C;
pub const WM_GETTEXT: u32          = 0x000D;
pub const WM_PAINT: u32            = 0x000F;
pub const WM_CLOSE: u32            = 0x0010;
pub const WM_QUIT: u32             = 0x0012;
pub const WM_ERASEBKGND: u32       = 0x0014;
pub const WM_SHOWWINDOW: u32       = 0x0018;
pub const WM_WINDOWPOSCHANGING: u32= 0x0046;
pub const WM_WINDOWPOSCHANGED: u32 = 0x0047;
pub const WM_KEYDOWN: u32          = 0x0100;
pub const WM_KEYUP: u32            = 0x0101;
pub const WM_CHAR: u32             = 0x0102;
pub const WM_COMMAND: u32          = 0x0111;
pub const WM_TIMER: u32            = 0x0113;
pub const WM_MOUSEMOVE: u32        = 0x0200;
pub const WM_LBUTTONDOWN: u32      = 0x0201;
pub const WM_LBUTTONUP: u32        = 0x0202;
pub const WM_RBUTTONDOWN: u32      = 0x0204;
pub const WM_RBUTTONUP: u32        = 0x0205;
pub const WM_USER: u32             = 0x0400;

// PeekMessage flags
pub const PM_NOREMOVE: u32         = 0x0000;
pub const PM_REMOVE: u32           = 0x0001;

// ============================================================================
// GDI Constants & Raster Operations (ROPs)
// ============================================================================

pub const SRCCOPY: u32             = 0x00CC0020;
pub const SRCPAINT: u32            = 0x00EE0086;
pub const SRCAND: u32              = 0x008800C6;
pub const SRCINVERT: u32           = 0x00660046;
pub const SRCERASE: u32            = 0x00440328;
pub const NOTSRCCOPY: u32          = 0x00330008;
pub const NOTSRCERASE: u32         = 0x001100A6;
pub const MERGECOPY: u32           = 0x00C000CA;
pub const MERGEPAINT: u32          = 0x00BB0226;
pub const PATCOPY: u32             = 0x00F00021;
pub const PATPAINT: u32            = 0x00FB0A09;
pub const PATINVERT: u32           = 0x005A0049;
pub const DSTINVERT: u32           = 0x00550009;
pub const BLACKNESS: u32           = 0x00000042;
pub const WHITENESS: u32           = 0x00FF0062;

// ExtTextOut Options
pub const ETO_OPAQUE: u32          = 0x0002;
pub const ETO_CLIPPED: u32         = 0x0004;

// Default colors (32-bit ARGB/XRGB 0x00RRGGBB)
pub const COLOR_BLACK: u32         = 0x00000000;
pub const COLOR_WHITE: u32         = 0x00FFFFFF;
pub const COLOR_RED: u32           = 0x00FF0000;
pub const COLOR_GREEN: u32         = 0x0000FF00;
pub const COLOR_BLUE: u32          = 0x000000FF;
pub const COLOR_GRAY: u32          = 0x00808080;
pub const COLOR_LIGHTGRAY: u32     = 0x00C0C0C0;
pub const COLOR_NAVY: u32          = 0x00000080;

// ============================================================================
// Core Win32 Types & Structures
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct HWND(pub usize);

impl HWND {
    pub const NULL: Self = HWND(0);
    pub const DESKTOP: Self = HWND(0x10000);
    pub const BROADCAST: Self = HWND(0xFFFF);

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct HDC(pub usize);

impl HDC {
    pub const NULL: Self = HDC(0);
    pub const SCREEN: Self = HDC(0x20000);

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct HBITMAP(pub usize);

impl HBITMAP {
    pub const NULL: Self = HBITMAP(0);

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct HBRUSH(pub usize);

impl HBRUSH {
    pub const NULL: Self = HBRUSH(0);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RECT {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self { left, top, right, bottom }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// Standard Windows Win32 MSG structure
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct MSG {
    pub hwnd: usize,       // HWND
    pub message: u32,      // UINT
    pub wParam: usize,     // WPARAM
    pub lParam: isize,     // LPARAM
    pub time: u32,         // DWORD (kernel uptime in ms)
    pub pt: POINT,         // POINT (cursor position)
}

impl Default for MSG {
    fn default() -> Self {
        Self {
            hwnd: 0,
            message: WM_NULL,
            wParam: 0,
            lParam: 0,
            time: 0,
            pt: POINT::default(),
        }
    }
}

/// Parameters for NtUserCreateWindowEx syscall
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CreateWindowParams {
    pub ex_style: u32,
    pub class_name_ptr: u64,
    pub window_name_ptr: u64,
    pub style: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub parent_hwnd: usize,
    pub menu_handle: usize,
    pub instance_handle: usize,
    pub param: u64,
}

/// Parameters for NtGdiBitBlt syscall
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BitBltParams {
    pub hdc_dest: usize,
    pub x_dest: i32,
    pub y_dest: i32,
    pub width: i32,
    pub height: i32,
    pub hdc_src: usize,
    pub x_src: i32,
    pub y_src: i32,
    pub rop: u32,
}

/// Parameters for NtUserSetWindowPos syscall
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WindowPosParams {
    pub hwnd: usize,
    pub hwnd_insert_after: usize,
    pub x: i32,
    pub y: i32,
    pub cx: i32,
    pub cy: i32,
    pub flags: u32,
}

// ============================================================================
// Safe User-Memory Access Utilities
// ============================================================================

pub unsafe fn write_user<T: Copy>(ptr: u64, val: &T) -> bool {
    if ptr == 0 {
        return false;
    }
    let target = ptr as *mut T;
    core::ptr::write_unaligned(target, *val);
    true
}

pub unsafe fn read_user<T: Copy>(ptr: u64) -> Option<T> {
    if ptr == 0 {
        return None;
    }
    let target = ptr as *const T;
    Some(core::ptr::read_unaligned(target))
}

pub unsafe fn read_user_string(ptr: u64, max_len: usize) -> Option<String> {
    if ptr == 0 {
        return None;
    }
    if ptr <= 0xFFFF {
        return Some(format!("#{}", ptr));
    }
    let byte_ptr = ptr as *const u8;
    let b0 = *byte_ptr;
    let b1 = *byte_ptr.add(1);

    // Heuristic: if second byte is 0 while first is non-zero, treat as UTF-16
    if b0 != 0 && b1 == 0 {
        let u16_ptr = ptr as *const u16;
        let mut u16_vec = Vec::new();
        for i in 0..max_len {
            let val = *u16_ptr.add(i);
            if val == 0 {
                break;
            }
            u16_vec.push(val);
        }
        return String::from_utf16(&u16_vec).ok();
    }

    // Default: null-terminated C-string / UTF-8
    let mut bytes = Vec::new();
    for i in 0..max_len {
        let b = *byte_ptr.add(i);
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).ok()
}

// ============================================================================
// Built-in 8x8 ASCII Font Renderer
// ============================================================================

/// Retrieves the 8x8 bitmap glyph for ASCII characters 32..126.
pub fn get_ascii_8x8_glyph(ch: char) -> [u8; 8] {
    match ch {
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '!' => [0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00],
        '"' => [0x66, 0x66, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00],
        '#' => [0x24, 0x7E, 0x24, 0x24, 0x7E, 0x24, 0x00, 0x00],
        '$' => [0x18, 0x3E, 0x58, 0x3C, 0x1A, 0x7C, 0x18, 0x00],
        '%' => [0x62, 0x64, 0x08, 0x10, 0x26, 0x46, 0x00, 0x00],
        '&' => [0x38, 0x44, 0x38, 0x54, 0x4A, 0x34, 0x00, 0x00],
        '\''=> [0x18, 0x18, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00],
        '(' => [0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00],
        ')' => [0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00],
        '*' => [0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00],
        '+' => [0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x08, 0x10],
        '-' => [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
        '/' => [0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x00],
        '0' => [0x3C, 0x46, 0x4A, 0x52, 0x62, 0x42, 0x3C, 0x00],
        '1' => [0x18, 0x28, 0x08, 0x08, 0x08, 0x08, 0x3E, 0x00],
        '2' => [0x3C, 0x42, 0x02, 0x0C, 0x30, 0x40, 0x7E, 0x00],
        '3' => [0x3C, 0x42, 0x02, 0x1C, 0x02, 0x42, 0x3C, 0x00],
        '4' => [0x08, 0x18, 0x28, 0x48, 0x7E, 0x08, 0x08, 0x00],
        '5' => [0x7E, 0x40, 0x7C, 0x02, 0x02, 0x42, 0x3C, 0x00],
        '6' => [0x3C, 0x42, 0x40, 0x7C, 0x42, 0x42, 0x3C, 0x00],
        '7' => [0x7E, 0x02, 0x04, 0x08, 0x10, 0x10, 0x10, 0x00],
        '8' => [0x3C, 0x42, 0x42, 0x3C, 0x42, 0x42, 0x3C, 0x00],
        '9' => [0x3C, 0x42, 0x42, 0x3E, 0x02, 0x42, 0x3C, 0x00],
        ':' => [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00],
        ';' => [0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x08, 0x10],
        '<' => [0x0C, 0x18, 0x30, 0x60, 0x30, 0x18, 0x0C, 0x00],
        '=' => [0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
        '>' => [0x30, 0x18, 0x0C, 0x06, 0x0C, 0x18, 0x30, 0x00],
        '?' => [0x3C, 0x42, 0x02, 0x0C, 0x18, 0x00, 0x18, 0x00],
        '@' => [0x3C, 0x42, 0x4E, 0x52, 0x4E, 0x40, 0x3C, 0x00],
        'A' => [0x18, 0x24, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00],
        'B' => [0x7C, 0x42, 0x42, 0x7C, 0x42, 0x42, 0x7C, 0x00],
        'C' => [0x3C, 0x42, 0x40, 0x40, 0x40, 0x42, 0x3C, 0x00],
        'D' => [0x78, 0x44, 0x42, 0x42, 0x42, 0x44, 0x78, 0x00],
        'E' => [0x7E, 0x40, 0x40, 0x78, 0x40, 0x40, 0x7E, 0x00],
        'F' => [0x7E, 0x40, 0x40, 0x78, 0x40, 0x40, 0x40, 0x00],
        'G' => [0x3C, 0x42, 0x40, 0x4E, 0x42, 0x42, 0x3C, 0x00],
        'H' => [0x42, 0x42, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00],
        'I' => [0x38, 0x10, 0x10, 0x10, 0x10, 0x10, 0x38, 0x00],
        'J' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x44, 0x38, 0x00],
        'K' => [0x44, 0x48, 0x50, 0x60, 0x50, 0x48, 0x44, 0x00],
        'L' => [0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x7E, 0x00],
        'M' => [0x81, 0xC3, 0xA5, 0x99, 0x81, 0x81, 0x81, 0x00],
        'N' => [0x42, 0x62, 0x52, 0x4A, 0x46, 0x42, 0x42, 0x00],
        'O' => [0x3C, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'P' => [0x7C, 0x42, 0x42, 0x7C, 0x40, 0x40, 0x40, 0x00],
        'Q' => [0x3C, 0x42, 0x42, 0x42, 0x4A, 0x44, 0x3A, 0x00],
        'R' => [0x7C, 0x42, 0x42, 0x7C, 0x48, 0x44, 0x42, 0x00],
        'S' => [0x3C, 0x42, 0x40, 0x3C, 0x02, 0x42, 0x3C, 0x00],
        'T' => [0x7E, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x00],
        'U' => [0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'V' => [0x42, 0x42, 0x42, 0x42, 0x24, 0x24, 0x18, 0x00],
        'W' => [0x81, 0x81, 0x81, 0x99, 0xA5, 0xC3, 0x81, 0x00],
        'X' => [0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x42, 0x00],
        'Y' => [0x42, 0x42, 0x24, 0x18, 0x10, 0x10, 0x10, 0x00],
        'Z' => [0x7E, 0x04, 0x08, 0x10, 0x20, 0x40, 0x7E, 0x00],
        '[' => [0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00],
        '\\'=> [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x00],
        ']' => [0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00],
        '^' => [0x10, 0x28, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00],
        '`' => [0x20, 0x10, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00],
        'a' => [0x00, 0x00, 0x3C, 0x02, 0x3E, 0x42, 0x3E, 0x00],
        'b' => [0x40, 0x40, 0x5C, 0x62, 0x42, 0x62, 0x5C, 0x00],
        'c' => [0x00, 0x00, 0x3C, 0x42, 0x40, 0x42, 0x3C, 0x00],
        'd' => [0x02, 0x02, 0x3A, 0x46, 0x42, 0x46, 0x3A, 0x00],
        'e' => [0x00, 0x00, 0x3C, 0x42, 0x7E, 0x40, 0x3C, 0x00],
        'f' => [0x1C, 0x22, 0x20, 0x78, 0x20, 0x20, 0x20, 0x00],
        'g' => [0x00, 0x00, 0x3E, 0x42, 0x42, 0x3E, 0x02, 0x3C],
        'h' => [0x40, 0x40, 0x5C, 0x62, 0x42, 0x42, 0x42, 0x00],
        'i' => [0x10, 0x00, 0x30, 0x10, 0x10, 0x10, 0x38, 0x00],
        'j' => [0x08, 0x00, 0x18, 0x08, 0x08, 0x08, 0x48, 0x30],
        'k' => [0x40, 0x40, 0x44, 0x48, 0x70, 0x48, 0x44, 0x00],
        'l' => [0x30, 0x10, 0x10, 0x10, 0x10, 0x10, 0x38, 0x00],
        'm' => [0x00, 0x00, 0x6A, 0x95, 0x95, 0x95, 0x95, 0x00],
        'n' => [0x00, 0x00, 0x5C, 0x62, 0x42, 0x42, 0x42, 0x00],
        'o' => [0x00, 0x00, 0x3C, 0x42, 0x42, 0x42, 0x3C, 0x00],
        'p' => [0x00, 0x00, 0x5C, 0x62, 0x42, 0x62, 0x5C, 0x40],
        'q' => [0x00, 0x00, 0x3A, 0x46, 0x42, 0x46, 0x3A, 0x02],
        'r' => [0x00, 0x00, 0x5C, 0x62, 0x40, 0x40, 0x40, 0x00],
        's' => [0x00, 0x00, 0x3E, 0x40, 0x3C, 0x02, 0x7C, 0x00],
        't' => [0x20, 0x20, 0x78, 0x20, 0x20, 0x22, 0x1C, 0x00],
        'u' => [0x00, 0x00, 0x42, 0x42, 0x42, 0x46, 0x3A, 0x00],
        'v' => [0x00, 0x00, 0x42, 0x42, 0x24, 0x24, 0x18, 0x00],
        'w' => [0x00, 0x00, 0x81, 0x99, 0x99, 0x5A, 0x24, 0x00],
        'x' => [0x00, 0x00, 0x42, 0x24, 0x18, 0x24, 0x42, 0x00],
        'y' => [0x00, 0x00, 0x42, 0x42, 0x42, 0x3E, 0x02, 0x3C],
        'z' => [0x00, 0x00, 0x7E, 0x08, 0x10, 0x20, 0x7E, 0x00],
        '{' => [0x0E, 0x18, 0x18, 0x70, 0x18, 0x18, 0x0E, 0x00],
        '|' => [0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
        '}' => [0x70, 0x18, 0x18, 0x0E, 0x18, 0x18, 0x70, 0x00],
        '~' => [0x32, 0x4C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        _   => [0xFF, 0x81, 0x81, 0x81, 0x81, 0x81, 0x81, 0xFF], // Square box for unknown
    }
}

// ============================================================================
// GDI Subsystem: Bitmaps, Brushes, and Device Contexts
// ============================================================================

/// In-kernel GDI Bitmap representation
#[derive(Debug, Clone)]
pub struct GdiBitmap {
    pub hbitmap: HBITMAP,
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
    pub pitch: u32,
    pub pixels: Vec<u32>,
}

impl GdiBitmap {
    pub fn new(hbitmap: HBITMAP, width: u32, height: u32, bpp: u32) -> Self {
        let pitch = width * (bpp / 8).max(1);
        let count = (width * height) as usize;
        let pixels = vec![0u32; count];
        Self {
            hbitmap,
            width,
            height,
            bpp,
            pitch,
            pixels,
        }
    }
}

/// In-kernel GDI Brush representation
#[derive(Debug, Clone)]
pub struct GdiBrush {
    pub hbrush: HBRUSH,
    pub color: u32,
    pub style: u32,
}

/// In-kernel Device Context (HDC)
#[derive(Debug, Clone)]
pub struct DeviceContext {
    pub hdc: HDC,
    pub is_screen: bool,
    pub width: u32,
    pub height: u32,
    pub selected_bitmap: Option<HBITMAP>,
    pub selected_brush: Option<HBRUSH>,
    pub text_color: u32,
    pub bg_color: u32,
    pub pen_color: u32,
    pub pen_width: u32,
    pub current_x: i32,
    pub current_y: i32,
    pub pixels: Vec<u32>,
}

impl DeviceContext {
    pub fn new(hdc: HDC, width: u32, height: u32, is_screen: bool) -> Self {
        let count = (width * height) as usize;
        let pixels = vec![COLOR_WHITE; count];
        Self {
            hdc,
            is_screen,
            width,
            height,
            selected_bitmap: None,
            selected_brush: None,
            text_color: COLOR_BLACK,
            bg_color: COLOR_WHITE,
            pen_color: COLOR_BLACK,
            pen_width: 1,
            current_x: 0,
            current_y: 0,
            pixels,
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let idx = (y as u32 * self.width + x as u32) as usize;
        if idx < self.pixels.len() {
            self.pixels[idx] = color;
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: i32, y: i32) -> Option<u32> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        let idx = (y as u32 * self.width + x as u32) as usize;
        self.pixels.get(idx).copied()
    }

    pub fn fill_rect(&mut self, left: i32, top: i32, right: i32, bottom: i32, color: u32) {
        let x0 = left.max(0).min(self.width as i32);
        let x1 = right.max(0).min(self.width as i32);
        let y0 = top.max(0).min(self.height as i32);
        let y1 = bottom.max(0).min(self.height as i32);

        for y in y0..y1 {
            let row_offset = (y as u32 * self.width) as usize;
            for x in x0..x1 {
                let idx = row_offset + x as usize;
                if idx < self.pixels.len() {
                    self.pixels[idx] = color;
                }
            }
        }
    }

    pub fn draw_rectangle(&mut self, left: i32, top: i32, right: i32, bottom: i32, pen_color: u32, fill_color: Option<u32>) {
        if let Some(fill) = fill_color {
            self.fill_rect(left + 1, top + 1, right - 1, bottom - 1, fill);
        }
        // Top and bottom borders
        for x in left..right {
            self.set_pixel(x, top, pen_color);
            self.set_pixel(x, bottom - 1, pen_color);
        }
        // Left and right borders
        for y in top..bottom {
            self.set_pixel(left, y, pen_color);
            self.set_pixel(right - 1, y, pen_color);
        }
    }

    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
        // Bresenham line algorithm
        let mut x = x0;
        let mut y = y0;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.set_pixel(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub fn ext_text_out(&mut self, start_x: i32, start_y: i32, text: &str, options: u32, fg_color: u32, bg_color: Option<u32>) {
        let mut cur_x = start_x;
        let mut cur_y = start_y;
        let opaque = (options & ETO_OPAQUE) != 0 || bg_color.is_some();
        let bg = bg_color.unwrap_or(self.bg_color);

        for ch in text.chars() {
            if ch == '\n' {
                cur_y += 10;
                cur_x = start_x;
                continue;
            }
            if ch == '\r' {
                continue;
            }
            let glyph = get_ascii_8x8_glyph(ch);
            for row in 0..8 {
                let y = cur_y + row as i32;
                let bits = glyph[row];
                for col in 0..8 {
                    let x = cur_x + col as i32;
                    let bit_on = (bits & (0x80 >> col)) != 0;
                    if bit_on {
                        self.set_pixel(x, y, fg_color);
                    } else if opaque {
                        self.set_pixel(x, y, bg);
                    }
                }
            }
            cur_x += 8;
        }
    }

    pub fn bit_blt(&mut self, dest_x: i32, dest_y: i32, width: i32, height: i32, src_pixels: &[u32], src_w: u32, src_h: u32, src_x: i32, src_y: i32, rop: u32) {
        if width <= 0 || height <= 0 {
            return;
        }

        for row in 0..height {
            let dy = dest_y + row;
            let sy = src_y + row;
            if dy < 0 || dy >= self.height as i32 {
                continue;
            }
            if sy < 0 || sy >= src_h as i32 {
                continue;
            }

            for col in 0..width {
                let dx = dest_x + col;
                let sx = src_x + col;
                if dx < 0 || dx >= self.width as i32 {
                    continue;
                }
                if sx < 0 || sx >= src_w as i32 {
                    continue;
                }

                let src_idx = (sy as u32 * src_w + sx as u32) as usize;
                let dst_idx = (dy as u32 * self.width + dx as u32) as usize;
                if src_idx >= src_pixels.len() || dst_idx >= self.pixels.len() {
                    continue;
                }

                let s = src_pixels[src_idx];
                let d = self.pixels[dst_idx];

                let result = match rop {
                    SRCCOPY => s,
                    SRCPAINT => d | s,
                    SRCAND => d & s,
                    SRCINVERT => d ^ s,
                    SRCERASE => s & (!d),
                    NOTSRCCOPY => !s,
                    NOTSRCERASE => !(d | s),
                    DSTINVERT => !d,
                    BLACKNESS => 0x00000000,
                    WHITENESS => 0x00FFFFFF,
                    _ => s,
                };

                self.pixels[dst_idx] = result;
            }
        }
    }
}

// ============================================================================
// Window Manager Subsystem: Window, Tree, and Thread Message Queue
// ============================================================================

/// In-kernel representation of a Win32 Window
#[derive(Debug, Clone)]
pub struct Window {
    pub hwnd: HWND,
    pub class_name: String,
    pub title: String,
    pub style: u32,
    pub ex_style: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub parent: Option<HWND>,
    pub children: Vec<HWND>,
    pub owner_pid: u32,
    pub owner_tid: u32,
    pub visible: bool,
    pub enabled: bool,
    pub hdc: Option<HDC>,
    pub wnd_proc: u64,
    pub paint_pending: bool,
}

impl Window {
    pub fn client_rect(&self) -> RECT {
        // Simple client rect: (0, 0, width, height)
        let border = if (self.style & WS_BORDER) != 0 || (self.style & WS_THICKFRAME) != 0 { 2 } else { 0 };
        let caption = if (self.style & WS_CAPTION) != 0 { 20 } else { 0 };
        let w = (self.width - 2 * border).max(0);
        let h = (self.height - border - caption).max(0);
        RECT::new(0, 0, w, h)
    }

    pub fn window_rect(&self) -> RECT {
        RECT::new(self.x, self.y, self.x + self.width, self.y + self.height)
    }
}

/// Global Window Tree managing hierarchy and Z-ordering
#[derive(Debug, Default)]
pub struct GlobalWindowTree {
    pub desktop_hwnd: HWND,
    pub z_order: Vec<HWND>, // Front to back (index 0 is topmost active window)
}

impl GlobalWindowTree {
    pub fn new(desktop: HWND) -> Self {
        Self {
            desktop_hwnd: desktop,
            z_order: vec![desktop],
        }
    }

    pub fn bring_to_top(&mut self, hwnd: HWND) {
        if let Some(pos) = self.z_order.iter().position(|&h| h == hwnd) {
            self.z_order.remove(pos);
        }
        self.z_order.insert(0, hwnd);
    }

    pub fn send_to_bottom(&mut self, hwnd: HWND) {
        if let Some(pos) = self.z_order.iter().position(|&h| h == hwnd) {
            self.z_order.remove(pos);
        }
        // Place just before desktop window
        let insert_idx = self.z_order.len().saturating_sub(1);
        self.z_order.insert(insert_idx, hwnd);
    }

    pub fn insert_after(&mut self, hwnd: HWND, after: HWND) {
        if let Some(pos) = self.z_order.iter().position(|&h| h == hwnd) {
            self.z_order.remove(pos);
        }
        if after.0 == HWND_TOP {
            self.z_order.insert(0, hwnd);
        } else if after.0 == HWND_BOTTOM {
            self.send_to_bottom(hwnd);
        } else if let Some(target_pos) = self.z_order.iter().position(|&h| h == after) {
            self.z_order.insert(target_pos + 1, hwnd);
        } else {
            self.z_order.insert(0, hwnd);
        }
    }

    pub fn remove(&mut self, hwnd: HWND) {
        self.z_order.retain(|&h| h != hwnd);
    }
}

/// Win32 Thread Message Queue
#[derive(Debug, Default)]
pub struct ThreadMessageQueue {
    pub queue: VecDeque<MSG>,
    pub paint_pending_windows: HashSet<HWND>,
    pub quit_posted: bool,
    pub exit_code: i32,
}

impl ThreadMessageQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn post_message(&mut self, msg: MSG) {
        if msg.message == WM_QUIT {
            self.quit_posted = true;
            self.exit_code = msg.wParam as i32;
        }
        self.queue.push_back(msg);
    }

    pub fn peek_message(&mut self, hwnd: HWND, min: u32, max: u32, remove: bool) -> Option<MSG> {
        let mut found_idx = None;
        for (i, msg) in self.queue.iter().enumerate() {
            let hwnd_match = hwnd.is_null() || msg.hwnd == hwnd.0;
            let range_match = (min == 0 && max == 0) || (msg.message >= min && msg.message <= max);
            if hwnd_match && range_match {
                found_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = found_idx {
            if remove {
                self.queue.remove(idx)
            } else {
                self.queue.get(idx).copied()
            }
        } else {
            None
        }
    }

    pub fn get_message(&mut self, hwnd: HWND, min: u32, max: u32) -> Option<MSG> {
        // 1. Check posted queue
        if let Some(msg) = self.peek_message(hwnd, min, max, true) {
            return Some(msg);
        }

        // 2. Synthesize WM_PAINT if any windows need redraw
        if !self.paint_pending_windows.is_empty() {
            let win = *self.paint_pending_windows.iter().next().unwrap();
            let hwnd_match = hwnd.is_null() || win == hwnd;
            if hwnd_match && (min == 0 && max == 0 || (WM_PAINT >= min && WM_PAINT <= max)) {
                self.paint_pending_windows.remove(&win);
                return Some(MSG {
                    hwnd: win.0,
                    message: WM_PAINT,
                    wParam: 0,
                    lParam: 0,
                    time: 0,
                    pt: POINT::default(),
                });
            }
        }

        None
    }
}

// ============================================================================
// Win32k Subsystem Global State
// ============================================================================

pub struct Win32kState {
    pub windows: HashMap<HWND, Window>,
    pub window_tree: GlobalWindowTree,
    pub thread_queues: HashMap<(u32, u32), ThreadMessageQueue>, // (pid, tid)
    pub device_contexts: HashMap<HDC, DeviceContext>,
    pub bitmaps: HashMap<HBITMAP, GdiBitmap>,
    pub brushes: HashMap<HBRUSH, GdiBrush>,
    pub next_hwnd: usize,
    pub next_hdc: usize,
    pub next_hbitmap: usize,
    pub next_hbrush: usize,
    pub desktop_hwnd: HWND,
    pub screen_hdc: HDC,
}

impl Win32kState {
    pub fn new() -> Self {
        let desktop_hwnd = HWND::DESKTOP;
        let screen_hdc = HDC::SCREEN;

        let mut windows = HashMap::new();
        let mut device_contexts = HashMap::new();

        // 1. Initialize Screen DC (1024x768 default resolution)
        let screen_dc = DeviceContext::new(screen_hdc, 1024, 768, true);
        device_contexts.insert(screen_hdc, screen_dc);

        // 2. Initialize Desktop Window
        let desktop = Window {
            hwnd: desktop_hwnd,
            class_name: "DesktopClass".to_string(),
            title: "HybridOS Desktop".to_string(),
            style: WS_VISIBLE | WS_POPUP | WS_CLIPCHILDREN,
            ex_style: 0,
            x: 0,
            y: 0,
            width: 1024,
            height: 768,
            parent: None,
            children: Vec::new(),
            owner_pid: 0,
            owner_tid: 0,
            visible: true,
            enabled: true,
            hdc: Some(screen_hdc),
            wnd_proc: 0,
            paint_pending: false,
        };
        windows.insert(desktop_hwnd, desktop);

        Self {
            windows,
            window_tree: GlobalWindowTree::new(desktop_hwnd),
            thread_queues: HashMap::new(),
            device_contexts,
            bitmaps: HashMap::new(),
            brushes: HashMap::new(),
            next_hwnd: 0x10001,
            next_hdc: 0x20001,
            next_hbitmap: 0x30001,
            next_hbrush: 0x40001,
            desktop_hwnd,
            screen_hdc,
        }
    }

    pub fn alloc_hwnd(&mut self) -> HWND {
        let h = HWND(self.next_hwnd);
        self.next_hwnd += 1;
        h
    }

    pub fn alloc_hdc(&mut self) -> HDC {
        let h = HDC(self.next_hdc);
        self.next_hdc += 1;
        h
    }

    pub fn alloc_hbitmap(&mut self) -> HBITMAP {
        let h = HBITMAP(self.next_hbitmap);
        self.next_hbitmap += 1;
        h
    }

    pub fn alloc_hbrush(&mut self) -> HBRUSH {
        let h = HBRUSH(self.next_hbrush);
        self.next_hbrush += 1;
        h
    }

    pub fn get_or_create_queue(&mut self, pid: u32, tid: u32) -> &mut ThreadMessageQueue {
        self.thread_queues.entry((pid, tid)).or_default()
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<HWND> {
        for &hwnd in &self.window_tree.z_order {
            if let Some(win) = self.windows.get(&hwnd) {
                if win.visible && win.window_rect().contains(x, y) {
                    return Some(hwnd);
                }
            }
        }
        Some(self.desktop_hwnd)
    }

    pub fn find_window(&self, class: Option<&str>, title: Option<&str>) -> Option<HWND> {
        for (&hwnd, win) in &self.windows {
            if hwnd == self.desktop_hwnd {
                continue;
            }
            let class_match = class.map_or(true, |c| win.class_name == c);
            let title_match = title.map_or(true, |t| win.title == t);
            if class_match && title_match {
                return Some(hwnd);
            }
        }
        None
    }
}

static WIN32K_STATE: OnceLock<Mutex<Win32kState>> = OnceLock::new();

pub fn get_win32k_state() -> &'static Mutex<Win32kState> {
    WIN32K_STATE.get_or_init(|| Mutex::new(Win32kState::new()))
}

pub fn lock_win32k_state() -> std::sync::MutexGuard<'static, Win32kState> {
    get_win32k_state().lock().unwrap_or_else(|e| e.into_inner())
}

// ============================================================================
// Win32k Kernel Implementation: Window Manager (User32)
// ============================================================================

/// NtUserCreateWindowEx (0x1000)
pub fn nt_user_create_window_ex(
    proc: &mut HybridProcess,
    ex_style: u32,
    class_name: &str,
    window_name: &str,
    style: u32,
    mut x: i32,
    mut y: i32,
    mut width: i32,
    mut height: i32,
    parent_hwnd: Option<HWND>,
) -> (NtStatus, HWND) {
    let mut state = lock_win32k_state();

    // Default dimensions if CW_USEDEFAULT passed
    if x == CW_USEDEFAULT {
        x = 50 + (state.windows.len() as i32 * 20) % 200;
    }
    if y == CW_USEDEFAULT {
        y = 50 + (state.windows.len() as i32 * 20) % 200;
    }
    if width == CW_USEDEFAULT || width <= 0 {
        width = 640;
    }
    if height == CW_USEDEFAULT || height <= 0 {
        height = 480;
    }

    let parent = parent_hwnd.unwrap_or(state.desktop_hwnd);
    let new_hwnd = state.alloc_hwnd();
    let is_visible = (style & WS_VISIBLE) != 0;

    // Create client DC for this window
    let win_hdc = state.alloc_hdc();
    let dc = DeviceContext::new(win_hdc, width as u32, height as u32, false);
    state.device_contexts.insert(win_hdc, dc);

    let window = Window {
        hwnd: new_hwnd,
        class_name: class_name.to_string(),
        title: window_name.to_string(),
        style,
        ex_style,
        x,
        y,
        width,
        height,
        parent: Some(parent),
        children: Vec::new(),
        owner_pid: proc.pid,
        owner_tid: 0,
        visible: is_visible,
        enabled: (style & WS_DISABLED) == 0,
        hdc: Some(win_hdc),
        wnd_proc: 0,
        paint_pending: is_visible,
    };

    state.windows.insert(new_hwnd, window);

    // Register child with parent
    if let Some(p) = state.windows.get_mut(&parent) {
        p.children.push(new_hwnd);
    }

    // Add to Z-order
    if is_visible {
        state.window_tree.bring_to_top(new_hwnd);
    } else {
        state.window_tree.send_to_bottom(new_hwnd);
    }

    // Post WM_CREATE message to process message queue
    let queue = state.get_or_create_queue(proc.pid, 0);
    queue.post_message(MSG {
        hwnd: new_hwnd.0,
        message: WM_CREATE,
        wParam: 0,
        lParam: 0,
        time: 1,
        pt: POINT { x, y },
    });

    if is_visible {
        queue.paint_pending_windows.insert(new_hwnd);
    }

    (NtStatus::SUCCESS, new_hwnd)
}

/// NtUserDestroyWindow (0x1001)
pub fn nt_user_destroy_window(hwnd: HWND) -> NtStatus {
    let mut state = lock_win32k_state();

    if hwnd == state.desktop_hwnd || !state.windows.contains_key(&hwnd) {
        return NtStatus::INVALID_HANDLE;
    }

    // Collect window and all child windows recursively
    let mut to_destroy = Vec::new();
    let mut stack = vec![hwnd];

    while let Some(h) = stack.pop() {
        to_destroy.push(h);
        if let Some(win) = state.windows.get(&h) {
            for &child in &win.children {
                stack.push(child);
            }
        }
    }

    // Remove from parent's children list
    if let Some(win) = state.windows.get(&hwnd) {
        if let Some(parent) = win.parent {
            if let Some(p) = state.windows.get_mut(&parent) {
                p.children.retain(|&c| c != hwnd);
            }
        }
    }

    // Clean up windows, associated DCs, and queues
    for h in to_destroy {
        state.window_tree.remove(h);
        if let Some(win) = state.windows.remove(&h) {
            if let Some(hdc) = win.hdc {
                state.device_contexts.remove(&hdc);
            }
            // Enqueue WM_DESTROY
            let queue = state.get_or_create_queue(win.owner_pid, win.owner_tid);
            queue.post_message(MSG {
                hwnd: h.0,
                message: WM_DESTROY,
                wParam: 0,
                lParam: 0,
                time: 1,
                pt: POINT::default(),
            });
            queue.paint_pending_windows.remove(&h);
        }
    }

    NtStatus::SUCCESS
}

/// NtUserShowWindow (0x1002)
pub fn nt_user_show_window(hwnd: HWND, cmd_show: u32) -> (NtStatus, bool) {
    let mut state = lock_win32k_state();

    let win = match state.windows.get_mut(&hwnd) {
        Some(w) => w,
        None => return (NtStatus::INVALID_HANDLE, false),
    };

    let prev_visible = win.visible;

    match cmd_show {
        SW_HIDE => {
            win.visible = false;
            win.style &= !WS_VISIBLE;
        }
        SW_SHOWNORMAL | SW_SHOW | SW_MAXIMIZE | SW_RESTORE => {
            win.visible = true;
            win.style |= WS_VISIBLE;
            win.paint_pending = true;
        }
        SW_MINIMIZE | SW_SHOWMINIMIZED | SW_SHOWMINNOACTIVE => {
            win.visible = true;
            win.style = (win.style | WS_VISIBLE | WS_MINIMIZE) & !WS_MAXIMIZE;
        }
        _ => {
            win.visible = true;
            win.style |= WS_VISIBLE;
        }
    }

    let owner_pid = win.owner_pid;
    let owner_tid = win.owner_tid;
    let is_now_visible = win.visible;

    if is_now_visible {
        state.window_tree.bring_to_top(hwnd);
        let queue = state.get_or_create_queue(owner_pid, owner_tid);
        queue.post_message(MSG {
            hwnd: hwnd.0,
            message: WM_SHOWWINDOW,
            wParam: 1,
            lParam: 0,
            time: 1,
            pt: POINT::default(),
        });
        queue.paint_pending_windows.insert(hwnd);
    } else {
        state.window_tree.send_to_bottom(hwnd);
        let queue = state.get_or_create_queue(owner_pid, owner_tid);
        queue.post_message(MSG {
            hwnd: hwnd.0,
            message: WM_SHOWWINDOW,
            wParam: 0,
            lParam: 0,
            time: 1,
            pt: POINT::default(),
        });
        queue.paint_pending_windows.remove(&hwnd);
    }

    (NtStatus::SUCCESS, prev_visible)
}

/// NtUserSetWindowPos (0x1003)
pub fn nt_user_set_window_pos(
    hwnd: HWND,
    insert_after: HWND,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    flags: u32,
) -> NtStatus {
    let mut state = lock_win32k_state();

    let (dc_to_resize, cx_to_resize, cy_to_resize) = {
        let win = match state.windows.get_mut(&hwnd) {
            Some(w) => w,
            None => return NtStatus::INVALID_HANDLE,
        };

        if (flags & SWP_NOMOVE) == 0 {
            win.x = x;
            win.y = y;
        }
        let resize_dc = if (flags & SWP_NOSIZE) == 0 && cx > 0 && cy > 0 {
            win.width = cx;
            win.height = cy;
            win.hdc
        } else {
            None
        };
        if (flags & SWP_SHOWWINDOW) != 0 {
            win.visible = true;
            win.style |= WS_VISIBLE;
        }
        if (flags & SWP_HIDEWINDOW) != 0 {
            win.visible = false;
            win.style &= !WS_VISIBLE;
        }
        (resize_dc, cx, cy)
    };

    if let Some(hdc) = dc_to_resize {
        if let Some(dc) = state.device_contexts.get_mut(&hdc) {
            dc.width = cx_to_resize as u32;
            dc.height = cy_to_resize as u32;
            let count = (cx_to_resize * cy_to_resize) as usize;
            dc.pixels.resize(count, COLOR_WHITE);
        }
    }

    if (flags & SWP_NOZORDER) == 0 {
        state.window_tree.insert_after(hwnd, insert_after);
    }

    NtStatus::SUCCESS
}

/// NtUserGetMessage (0x1004)
pub fn nt_user_get_message(
    proc: &HybridProcess,
    lp_msg: u64,
    hwnd: HWND,
    min_filter: u32,
    max_filter: u32,
) -> NtStatus {
    let mut state = lock_win32k_state();
    let queue = state.get_or_create_queue(proc.pid, 0);

    if let Some(msg) = queue.get_message(hwnd, min_filter, max_filter) {
        unsafe {
            if !write_user(lp_msg, &msg) {
                return NtStatus::INVALID_PARAMETER;
            }
        }
        if msg.message == WM_QUIT {
            NtStatus(0) // Win32 GetMessage returns FALSE (0) on WM_QUIT
        } else {
            NtStatus(1) // Win32 GetMessage returns TRUE (1) on normal message
        }
    } else {
        // Return 0 if queue empty
        NtStatus(0)
    }
}

/// NtUserPeekMessage (0x1005)
pub fn nt_user_peek_message(
    proc: &HybridProcess,
    lp_msg: u64,
    hwnd: HWND,
    min_filter: u32,
    max_filter: u32,
    remove_flag: u32,
) -> NtStatus {
    let mut state = lock_win32k_state();
    let queue = state.get_or_create_queue(proc.pid, 0);
    let remove = (remove_flag & PM_REMOVE) != 0;

    if let Some(msg) = queue.peek_message(hwnd, min_filter, max_filter, remove) {
        unsafe {
            if !write_user(lp_msg, &msg) {
                return NtStatus::INVALID_PARAMETER;
            }
        }
        NtStatus(1) // Message retrieved
    } else {
        NtStatus(0) // No message available
    }
}

/// NtUserPostMessage (0x1006)
pub fn nt_user_post_message(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> NtStatus {
    let mut state = lock_win32k_state();

    let new_msg = MSG {
        hwnd: hwnd.0,
        message: msg,
        wParam: wparam,
        lParam: lparam,
        time: 1,
        pt: POINT::default(),
    };

    if hwnd == HWND::BROADCAST {
        // Post to all active threads/windows
        let pids: Vec<u32> = state.windows.values().map(|w| w.owner_pid).collect();
        for pid in pids {
            state.get_or_create_queue(pid, 0).post_message(new_msg);
        }
        NtStatus::SUCCESS
    } else if let Some(win) = state.windows.get(&hwnd) {
        let pid = win.owner_pid;
        let tid = win.owner_tid;
        state.get_or_create_queue(pid, tid).post_message(new_msg);
        NtStatus::SUCCESS
    } else {
        NtStatus::INVALID_HANDLE
    }
}

/// NtUserDispatchMessage (0x1007)
pub fn nt_user_dispatch_message(lp_msg: u64) -> (NtStatus, isize) {
    let msg: MSG = match unsafe { read_user(lp_msg) } {
        Some(m) => m,
        None => return (NtStatus::INVALID_PARAMETER, 0),
    };

    let mut state = lock_win32k_state();
    let hwnd = HWND(msg.hwnd);

    if let Some(win) = state.windows.get_mut(&hwnd) {
        match msg.message {
            WM_PAINT => {
                win.paint_pending = false;
            }
            WM_CLOSE => {
                // Default action for WM_CLOSE: post WM_DESTROY
                let pid = win.owner_pid;
                let tid = win.owner_tid;
                state.get_or_create_queue(pid, tid).post_message(MSG {
                    hwnd: msg.hwnd,
                    message: WM_DESTROY,
                    wParam: 0,
                    lParam: 0,
                    time: 1,
                    pt: POINT::default(),
                });
            }
            _ => {}
        }
    }

    (NtStatus::SUCCESS, 0)
}

// ============================================================================
// Win32k Kernel Implementation: GDI Subsystem
// ============================================================================

/// NtGdiCreateCompatibleDC (0x1010)
pub fn nt_gdi_create_compatible_dc(existing_hdc: HDC) -> (NtStatus, HDC) {
    let mut state = lock_win32k_state();

    let (w, h) = if existing_hdc.is_null() || existing_hdc == HDC::SCREEN {
        (1024, 768)
    } else if let Some(dc) = state.device_contexts.get(&existing_hdc) {
        (dc.width, dc.height)
    } else {
        (1024, 768)
    };

    let new_hdc = state.alloc_hdc();
    let new_dc = DeviceContext::new(new_hdc, w, h, false);
    state.device_contexts.insert(new_hdc, new_dc);

    (NtStatus::SUCCESS, new_hdc)
}

/// NtGdiDeleteDC (0x1011)
pub fn nt_gdi_delete_dc(hdc: HDC) -> NtStatus {
    let mut state = lock_win32k_state();

    if hdc == state.screen_hdc || hdc.is_null() {
        return NtStatus::INVALID_PARAMETER;
    }

    if state.device_contexts.remove(&hdc).is_some() {
        NtStatus::SUCCESS
    } else {
        NtStatus::INVALID_HANDLE
    }
}

/// NtGdiCreateBitmap (0x1012)
pub fn nt_gdi_create_bitmap(width: u32, height: u32, _planes: u32, bpp: u32, bits_ptr: u64) -> (NtStatus, HBITMAP) {
    if width == 0 || height == 0 {
        return (NtStatus::INVALID_PARAMETER, HBITMAP::NULL);
    }

    let mut state = lock_win32k_state();
    let hbitmap = state.alloc_hbitmap();
    let mut bitmap = GdiBitmap::new(hbitmap, width, height, if bpp == 0 { 32 } else { bpp });

    if bits_ptr != 0 {
        let total_pixels = (width * height) as usize;
        let user_slice = unsafe { core::slice::from_raw_parts(bits_ptr as *const u32, total_pixels) };
        bitmap.pixels.copy_from_slice(user_slice);
    }

    state.bitmaps.insert(hbitmap, bitmap);
    (NtStatus::SUCCESS, hbitmap)
}

/// NtGdiCreateCompatibleBitmap (0x1013)
pub fn nt_gdi_create_compatible_bitmap(hdc: HDC, width: u32, height: u32) -> (NtStatus, HBITMAP) {
    if width == 0 || height == 0 {
        return (NtStatus::INVALID_PARAMETER, HBITMAP::NULL);
    }

    let mut state = lock_win32k_state();
    let bpp = if let Some(dc) = state.device_contexts.get(&hdc) {
        if dc.is_screen { 32 } else { 32 }
    } else {
        32
    };

    let hbitmap = state.alloc_hbitmap();
    let bitmap = GdiBitmap::new(hbitmap, width, height, bpp);
    state.bitmaps.insert(hbitmap, bitmap);

    (NtStatus::SUCCESS, hbitmap)
}

/// NtGdiSelectBitmap (0x1014)
pub fn nt_gdi_select_bitmap(hdc: HDC, hbitmap: HBITMAP) -> (NtStatus, HBITMAP) {
    let mut state = lock_win32k_state();

    let prev_bitmap = if let Some(dc) = state.device_contexts.get(&hdc) {
        dc.selected_bitmap.unwrap_or(HBITMAP::NULL)
    } else {
        return (NtStatus::INVALID_HANDLE, HBITMAP::NULL);
    };

    // If selecting a valid bitmap, copy dimensions and pixels into DC
    if hbitmap != HBITMAP::NULL {
        if let Some(bmp) = state.bitmaps.get(&hbitmap) {
            let w = bmp.width;
            let h = bmp.height;
            let pixels = bmp.pixels.clone();

            if let Some(dc) = state.device_contexts.get_mut(&hdc) {
                dc.selected_bitmap = Some(hbitmap);
                dc.width = w;
                dc.height = h;
                dc.pixels = pixels;
            }
        } else {
            return (NtStatus::INVALID_HANDLE, HBITMAP::NULL);
        }
    } else if let Some(dc) = state.device_contexts.get_mut(&hdc) {
        dc.selected_bitmap = None;
    }

    (NtStatus::SUCCESS, prev_bitmap)
}

/// NtGdiBitBlt (0x1015)
pub fn nt_gdi_bit_blt(
    hdc_dest: HDC,
    x_dest: i32,
    y_dest: i32,
    width: i32,
    height: i32,
    hdc_src: HDC,
    x_src: i32,
    y_src: i32,
    rop: u32,
) -> NtStatus {
    let mut state = lock_win32k_state();

    // Clone source pixels and dimensions to release borrow
    let (src_pixels, src_w, src_h) = match state.device_contexts.get(&hdc_src) {
        Some(dc) => (dc.pixels.clone(), dc.width, dc.height),
        None => return NtStatus::INVALID_HANDLE,
    };

    let dest_dc = match state.device_contexts.get_mut(&hdc_dest) {
        Some(dc) => dc,
        None => return NtStatus::INVALID_HANDLE,
    };

    dest_dc.bit_blt(x_dest, y_dest, width, height, &src_pixels, src_w, src_h, x_src, y_src, rop);

    // If a bitmap is selected in dest DC, sync the bitmap pixels
    if let Some(bmp_handle) = dest_dc.selected_bitmap {
        let pixels_clone = dest_dc.pixels.clone();
        if let Some(bmp) = state.bitmaps.get_mut(&bmp_handle) {
            bmp.pixels = pixels_clone;
        }
    }

    NtStatus::SUCCESS
}

/// NtGdiExtTextOutW (0x1016)
pub fn nt_gdi_ext_text_out_w(
    hdc: HDC,
    x: i32,
    y: i32,
    options: u32,
    string_ptr: u64,
    cwc: u32,
) -> NtStatus {
    if string_ptr == 0 || cwc == 0 {
        return NtStatus::INVALID_PARAMETER;
    }

    let text = match unsafe { read_user_string(string_ptr, cwc as usize) } {
        Some(s) => s,
        None => return NtStatus::INVALID_PARAMETER,
    };

    let mut state = lock_win32k_state();
    let dc = match state.device_contexts.get_mut(&hdc) {
        Some(d) => d,
        None => return NtStatus::INVALID_HANDLE,
    };

    let fg = dc.text_color;
    let bg = if (options & ETO_OPAQUE) != 0 { Some(dc.bg_color) } else { None };
    dc.ext_text_out(x, y, &text, options, fg, bg);

    // Sync bitmap if selected
    if let Some(bmp_handle) = dc.selected_bitmap {
        let pixels_clone = dc.pixels.clone();
        if let Some(bmp) = state.bitmaps.get_mut(&bmp_handle) {
            bmp.pixels = pixels_clone;
        }
    }

    NtStatus::SUCCESS
}

/// NtGdiRectangle (0x1017)
pub fn nt_gdi_rectangle(
    hdc: HDC,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    color: u32,
) -> NtStatus {
    let mut state = lock_win32k_state();

    let (pen, brush) = {
        let (pen_color, selected_brush, bg_color) = match state.device_contexts.get(&hdc) {
            Some(d) => (d.pen_color, d.selected_brush, d.bg_color),
            None => return NtStatus::INVALID_HANDLE,
        };

        let pen = if color != 0 { color } else { pen_color };
        let brush = if let Some(hbrush) = selected_brush {
            state.brushes.get(&hbrush).map(|b| b.color)
        } else {
            Some(bg_color)
        };
        (pen, brush)
    };

    let (selected_bmp, pixels_clone) = {
        let dc = state.device_contexts.get_mut(&hdc).unwrap();
        dc.draw_rectangle(left, top, right, bottom, pen, brush);
        (dc.selected_bitmap, dc.pixels.clone())
    };

    // Sync bitmap if selected
    if let Some(bmp_handle) = selected_bmp {
        if let Some(bmp) = state.bitmaps.get_mut(&bmp_handle) {
            bmp.pixels = pixels_clone;
        }
    }

    NtStatus::SUCCESS
}

// ============================================================================
// Extended GDI Primitives
// ============================================================================

/// NtGdiSetPixel (0x1018)
pub fn nt_gdi_set_pixel(hdc: HDC, x: i32, y: i32, color: u32) -> NtStatus {
    let mut state = lock_win32k_state();
    let dc = match state.device_contexts.get_mut(&hdc) {
        Some(d) => d,
        None => return NtStatus::INVALID_HANDLE,
    };
    dc.set_pixel(x, y, color);
    NtStatus::SUCCESS
}

/// NtGdiGetPixel (0x1019)
pub fn nt_gdi_get_pixel(hdc: HDC, x: i32, y: i32) -> (NtStatus, u32) {
    let state = lock_win32k_state();
    let dc = match state.device_contexts.get(&hdc) {
        Some(d) => d,
        None => return (NtStatus::INVALID_HANDLE, 0),
    };
    match dc.get_pixel(x, y) {
        Some(c) => (NtStatus::SUCCESS, c),
        None => (NtStatus::INVALID_PARAMETER, 0),
    }
}

/// NtGdiCreateSolidBrush (0x101A)
pub fn nt_gdi_create_solid_brush(color: u32) -> (NtStatus, HBRUSH) {
    let mut state = lock_win32k_state();
    let hbrush = state.alloc_hbrush();
    let brush = GdiBrush {
        hbrush,
        color,
        style: 0,
    };
    state.brushes.insert(hbrush, brush);
    (NtStatus::SUCCESS, hbrush)
}

/// NtGdiSelectBrush (0x101B)
pub fn nt_gdi_select_brush(hdc: HDC, hbrush: HBRUSH) -> (NtStatus, HBRUSH) {
    let mut state = lock_win32k_state();
    let prev = if let Some(dc) = state.device_contexts.get(&hdc) {
        dc.selected_brush.unwrap_or(HBRUSH::NULL)
    } else {
        return (NtStatus::INVALID_HANDLE, HBRUSH::NULL);
    };

    if let Some(dc) = state.device_contexts.get_mut(&hdc) {
        dc.selected_brush = Some(hbrush);
    }
    (NtStatus::SUCCESS, prev)
}

/// NtGdiFillRect (0x101C)
pub fn nt_gdi_fill_rect(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32, color: u32) -> NtStatus {
    let mut state = lock_win32k_state();
    let dc = match state.device_contexts.get_mut(&hdc) {
        Some(d) => d,
        None => return NtStatus::INVALID_HANDLE,
    };
    dc.fill_rect(left, top, right, bottom, color);
    NtStatus::SUCCESS
}

/// NtGdiLineTo (0x101D)
pub fn nt_gdi_line_to(hdc: HDC, x: i32, y: i32, color: u32) -> NtStatus {
    let mut state = lock_win32k_state();
    let dc = match state.device_contexts.get_mut(&hdc) {
        Some(d) => d,
        None => return NtStatus::INVALID_HANDLE,
    };
    let x0 = dc.current_x;
    let y0 = dc.current_y;
    dc.draw_line(x0, y0, x, y, color);
    dc.current_x = x;
    dc.current_y = y;
    NtStatus::SUCCESS
}

// ============================================================================
// Shadow SSDT Dispatch Table (0x1000..0x1200)
// ============================================================================

/// Universal Win32k Shadow SSDT Syscall Dispatcher
pub fn dispatch_win32k_syscall(
    proc: &mut HybridProcess,
    service_id: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
) -> NtStatus {
    match service_id {
        SYS_NT_USER_CREATE_WINDOW_EX => {
            // Check if passed via CreateWindowParams struct pointer in a1
            if a1 > 0x10000 && a2 == 0 && a3 == 0 {
                if let Some(params) = unsafe { read_user::<CreateWindowParams>(a1) } {
                    let class_str = unsafe { read_user_string(params.class_name_ptr, 128) }.unwrap_or_else(|| "DefaultClass".into());
                    let title_str = unsafe { read_user_string(params.window_name_ptr, 128) }.unwrap_or_else(|| "Window".into());
                    let parent = if params.parent_hwnd != 0 { Some(HWND(params.parent_hwnd)) } else { None };

                    let (status, hwnd) = nt_user_create_window_ex(
                        proc,
                        params.ex_style,
                        &class_str,
                        &title_str,
                        params.style,
                        params.x,
                        params.y,
                        params.width,
                        params.height,
                        parent,
                    );
                    if status.is_success() {
                        return NtStatus(hwnd.0 as i32);
                    }
                    return status;
                }
            }

            // Unpack from standard register arguments:
            // a1: ex_style
            // a2: lpClassName pointer
            // a3: lpWindowName pointer
            // a4: dwStyle
            // a5: coordinates ([i32; 4] ptr, packed, or x/y)
            // a6: parent_hwnd or out HWND ptr
            let ex_style = a1 as u32;
            let class_str = if a2 != 0 {
                unsafe { read_user_string(a2, 128) }.unwrap_or_else(|| "Button".into())
            } else {
                "DefaultClass".to_string()
            };
            let title_str = if a3 != 0 {
                unsafe { read_user_string(a3, 128) }.unwrap_or_else(|| "Window".into())
            } else {
                "DefaultTitle".to_string()
            };
            let style = a4 as u32;

            let (x, y, w, h) = if a5 > 0x10000 {
                if let Some(coords) = unsafe { read_user::<[i32; 4]>(a5) } {
                    (coords[0], coords[1], coords[2], coords[3])
                } else {
                    (CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT)
                }
            } else if a5 != 0 {
                let px = (a5 >> 32) as i32;
                let py = (a5 & 0xFFFFFFFF) as i32;
                let pw = (a6 >> 32) as i32;
                let ph = (a6 & 0xFFFFFFFF) as i32;
                (px, py, pw, ph)
            } else {
                (CW_USEDEFAULT, CW_USEDEFAULT, 640, 480)
            };

            let parent = if a6 != 0 && a6 <= 0x10000 { Some(HWND(a6 as usize)) } else { None };

            let (status, hwnd) = nt_user_create_window_ex(proc, ex_style, &class_str, &title_str, style, x, y, w, h, parent);

            if a6 > 0x10000 {
                unsafe { write_user(a6, &hwnd.0); }
            }

            if status.is_success() {
                NtStatus(hwnd.0 as i32)
            } else {
                status
            }
        }

        SYS_NT_USER_DESTROY_WINDOW => {
            let hwnd = HWND(a1 as usize);
            nt_user_destroy_window(hwnd)
        }

        SYS_NT_USER_SHOW_WINDOW => {
            let hwnd = HWND(a1 as usize);
            let cmd_show = a2 as u32;
            let (status, prev) = nt_user_show_window(hwnd, cmd_show);
            if status.is_success() {
                NtStatus(if prev { 1 } else { 0 })
            } else {
                status
            }
        }

        SYS_NT_USER_SET_WINDOW_POS => {
            if a1 > 0x10000 && a2 == 0 {
                if let Some(p) = unsafe { read_user::<WindowPosParams>(a1) } {
                    return nt_user_set_window_pos(HWND(p.hwnd), HWND(p.hwnd_insert_after), p.x, p.y, p.cx, p.cy, p.flags);
                }
            }
            let hwnd = HWND(a1 as usize);
            let insert_after = HWND(a2 as usize);
            if a6 > 0 {
                // a3 = x, a4 = y, a5 = cx, a6 = cy
                nt_user_set_window_pos(hwnd, insert_after, a3 as i32, a4 as i32, a5 as i32, a6 as i32, 0)
            } else {
                // a3 = packed_xy, a4 = packed_wh, a5 = flags
                let x = (a3 >> 32) as i32;
                let y = (a3 & 0xFFFFFFFF) as i32;
                let cx = (a4 >> 32) as i32;
                let cy = (a4 & 0xFFFFFFFF) as i32;
                let flags = a5 as u32;
                nt_user_set_window_pos(hwnd, insert_after, x, y, cx, cy, flags)
            }
        }

        SYS_NT_USER_GET_MESSAGE => {
            let lp_msg = a1;
            let hwnd = HWND(a2 as usize);
            let min_filter = a3 as u32;
            let max_filter = a4 as u32;
            nt_user_get_message(proc, lp_msg, hwnd, min_filter, max_filter)
        }

        SYS_NT_USER_PEEK_MESSAGE => {
            let lp_msg = a1;
            let hwnd = HWND(a2 as usize);
            let min_filter = a3 as u32;
            let max_filter = a4 as u32;
            let remove_flag = a5 as u32;
            nt_user_peek_message(proc, lp_msg, hwnd, min_filter, max_filter, remove_flag)
        }

        SYS_NT_USER_POST_MESSAGE => {
            let hwnd = HWND(a1 as usize);
            let msg = a2 as u32;
            let wparam = a3 as usize;
            let lparam = a4 as isize;
            nt_user_post_message(hwnd, msg, wparam, lparam)
        }

        SYS_NT_USER_DISPATCH_MESSAGE => {
            let lp_msg = a1;
            let (status, lres) = nt_user_dispatch_message(lp_msg);
            if status.is_success() {
                NtStatus(lres as i32)
            } else {
                status
            }
        }

        SYS_NT_USER_GET_DC => {
            let hwnd = HWND(a1 as usize);
            let state = lock_win32k_state();
            if let Some(win) = state.windows.get(&hwnd) {
                if let Some(hdc) = win.hdc {
                    return NtStatus(hdc.0 as i32);
                }
            }
            NtStatus::INVALID_HANDLE
        }

        SYS_NT_USER_RELEASE_DC => {
            NtStatus::SUCCESS
        }

        SYS_NT_USER_GET_CLIENT_RECT => {
            let hwnd = HWND(a1 as usize);
            let rect_ptr = a2;
            let state = lock_win32k_state();
            if let Some(win) = state.windows.get(&hwnd) {
                let r = win.client_rect();
                unsafe { write_user(rect_ptr, &r); }
                NtStatus::SUCCESS
            } else {
                NtStatus::INVALID_HANDLE
            }
        }

        SYS_NT_USER_GET_WINDOW_RECT => {
            let hwnd = HWND(a1 as usize);
            let rect_ptr = a2;
            let state = lock_win32k_state();
            if let Some(win) = state.windows.get(&hwnd) {
                let r = win.window_rect();
                unsafe { write_user(rect_ptr, &r); }
                NtStatus::SUCCESS
            } else {
                NtStatus::INVALID_HANDLE
            }
        }

        SYS_NT_USER_FIND_WINDOW_EX => {
            let class_str = if a2 != 0 { unsafe { read_user_string(a2, 128) } } else { None };
            let title_str = if a3 != 0 { unsafe { read_user_string(a3, 128) } } else { None };
            let state = lock_win32k_state();
            if let Some(h) = state.find_window(class_str.as_deref(), title_str.as_deref()) {
                NtStatus(h.0 as i32)
            } else {
                NtStatus(0)
            }
        }

        SYS_NT_USER_GET_DESKTOP_WINDOW => {
            let state = lock_win32k_state();
            NtStatus(state.desktop_hwnd.0 as i32)
        }

        SYS_NT_GDI_CREATE_COMPATIBLE_DC => {
            let hdc = HDC(a1 as usize);
            let (status, new_dc) = nt_gdi_create_compatible_dc(hdc);
            if status.is_success() {
                if a2 > 0 {
                    unsafe { write_user(a2, &new_dc.0); }
                }
                NtStatus(new_dc.0 as i32)
            } else {
                status
            }
        }

        SYS_NT_GDI_DELETE_DC => {
            let hdc = HDC(a1 as usize);
            nt_gdi_delete_dc(hdc)
        }

        SYS_NT_GDI_CREATE_BITMAP => {
            let width = a1 as u32;
            let height = a2 as u32;
            let planes = a3 as u32;
            let bpp = a4 as u32;
            let bits_ptr = a5;
            let (status, hbmp) = nt_gdi_create_bitmap(width, height, planes, bpp, bits_ptr);
            if status.is_success() {
                if a6 > 0 {
                    unsafe { write_user(a6, &hbmp.0); }
                }
                NtStatus(hbmp.0 as i32)
            } else {
                status
            }
        }

        SYS_NT_GDI_CREATE_COMPATIBLE_BITMAP => {
            let hdc = HDC(a1 as usize);
            let width = a2 as u32;
            let height = a3 as u32;
            let (status, hbmp) = nt_gdi_create_compatible_bitmap(hdc, width, height);
            if status.is_success() {
                if a4 > 0 {
                    unsafe { write_user(a4, &hbmp.0); }
                }
                NtStatus(hbmp.0 as i32)
            } else {
                status
            }
        }

        SYS_NT_GDI_SELECT_BITMAP => {
            let hdc = HDC(a1 as usize);
            let hbitmap = HBITMAP(a2 as usize);
            let (status, prev) = nt_gdi_select_bitmap(hdc, hbitmap);
            if status.is_success() {
                NtStatus(prev.0 as i32)
            } else {
                status
            }
        }

        SYS_NT_GDI_BIT_BLT => {
            if a1 > 0x10000 && a2 == 0 {
                if let Some(p) = unsafe { read_user::<BitBltParams>(a1) } {
                    return nt_gdi_bit_blt(HDC(p.hdc_dest), p.x_dest, p.y_dest, p.width, p.height, HDC(p.hdc_src), p.x_src, p.y_src, p.rop);
                }
            }
            let hdc_dest = HDC(a1 as usize);
            let x_dest = (a2 >> 32) as i32;
            let y_dest = (a2 & 0xFFFFFFFF) as i32;
            let width = (a3 >> 32) as i32;
            let height = (a3 & 0xFFFFFFFF) as i32;
            let hdc_src = HDC(a4 as usize);
            let x_src = (a5 >> 32) as i32;
            let y_src = (a5 & 0xFFFFFFFF) as i32;
            let rop = a6 as u32;
            nt_gdi_bit_blt(hdc_dest, x_dest, y_dest, width, height, hdc_src, x_src, y_src, rop)
        }

        SYS_NT_GDI_EXT_TEXT_OUT_W => {
            let hdc = HDC(a1 as usize);
            let x = a2 as i32;
            let y = a3 as i32;
            let options = a4 as u32;
            let string_ptr = a5;
            let cwc = a6 as u32;
            nt_gdi_ext_text_out_w(hdc, x, y, options, string_ptr, cwc)
        }

        SYS_NT_GDI_RECTANGLE => {
            let hdc = HDC(a1 as usize);
            let left = a2 as i32;
            let top = a3 as i32;
            let right = a4 as i32;
            let bottom = a5 as i32;
            let color = a6 as u32;
            nt_gdi_rectangle(hdc, left, top, right, bottom, color)
        }

        SYS_NT_GDI_SET_PIXEL => {
            let hdc = HDC(a1 as usize);
            let x = a2 as i32;
            let y = a3 as i32;
            let color = a4 as u32;
            nt_gdi_set_pixel(hdc, x, y, color)
        }

        SYS_NT_GDI_GET_PIXEL => {
            let hdc = HDC(a1 as usize);
            let x = a2 as i32;
            let y = a3 as i32;
            let (status, color) = nt_gdi_get_pixel(hdc, x, y);
            if status.is_success() {
                NtStatus(color as i32)
            } else {
                status
            }
        }

        SYS_NT_GDI_CREATE_SOLID_BRUSH => {
            let color = a1 as u32;
            let (status, hbrush) = nt_gdi_create_solid_brush(color);
            if status.is_success() {
                NtStatus(hbrush.0 as i32)
            } else {
                status
            }
        }

        SYS_NT_GDI_SELECT_BRUSH => {
            let hdc = HDC(a1 as usize);
            let hbrush = HBRUSH(a2 as usize);
            let (status, prev) = nt_gdi_select_brush(hdc, hbrush);
            if status.is_success() {
                NtStatus(prev.0 as i32)
            } else {
                status
            }
        }

        SYS_NT_GDI_FILL_RECT => {
            let hdc = HDC(a1 as usize);
            let left = a2 as i32;
            let top = a3 as i32;
            let right = a4 as i32;
            let bottom = a5 as i32;
            let color = a6 as u32;
            nt_gdi_fill_rect(hdc, left, top, right, bottom, color)
        }

        SYS_NT_GDI_LINE_TO => {
            let hdc = HDC(a1 as usize);
            let x = a2 as i32;
            let y = a3 as i32;
            let color = a4 as u32;
            nt_gdi_line_to(hdc, x, y, color)
        }

        // Stubbed / unsupported within 0x1000..0x1200 range
        _ => {
            if service_id >= WIN32K_SSDT_MIN && service_id <= WIN32K_SSDT_MAX {
                println!("[Win32k SSDT] Unimplemented Win32k service: 0x{:04X}", service_id);
                NtStatus::NOT_IMPLEMENTED
            } else {
                NtStatus::NOT_IMPLEMENTED
            }
        }
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::personality::ExecutionPersonality;

    fn make_test_proc() -> HybridProcess {
        HybridProcess::new(4001, "explorer.exe", ExecutionPersonality::WindowsNt)
    }

    #[test]
    fn test_window_creation_and_properties() {
        let mut proc = make_test_proc();

        let (status, hwnd) = nt_user_create_window_ex(
            &mut proc,
            WS_EX_APPWINDOW,
            "MainWindowClass",
            "HybridOS Desktop Manager",
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100,
            100,
            800,
            600,
            None,
        );

        assert_eq!(status, NtStatus::SUCCESS);
        assert!(!hwnd.is_null());

        let state = lock_win32k_state();
        let win = state.windows.get(&hwnd).expect("Window must exist in state");
        assert_eq!(win.class_name, "MainWindowClass");
        assert_eq!(win.title, "HybridOS Desktop Manager");
        assert_eq!(win.width, 800);
        assert_eq!(win.height, 600);
        assert!(win.visible);
        assert!(win.hdc.is_some());

        // Check window tree Z-order
        assert!(state.window_tree.z_order.contains(&hwnd));
    }

    #[test]
    fn test_window_hierarchy_and_cascade_destruction() {
        let mut proc = make_test_proc();

        // 1. Create parent window
        let (s1, parent) = nt_user_create_window_ex(
            &mut proc,
            0,
            "ParentClass",
            "Parent Window",
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0, 0, 500, 500,
            None,
        );
        assert!(s1.is_success());

        // 2. Create child button window
        let (s2, child) = nt_user_create_window_ex(
            &mut proc,
            0,
            "BUTTON",
            "OK Button",
            WS_CHILD | WS_VISIBLE,
            10, 10, 100, 30,
            Some(parent),
        );
        assert!(s2.is_success());

        {
            let state = lock_win32k_state();
            let parent_win = state.windows.get(&parent).unwrap();
            assert!(parent_win.children.contains(&child));
            let child_win = state.windows.get(&child).unwrap();
            assert_eq!(child_win.parent, Some(parent));
        }

        // 3. Destroy parent -> cascade destroys child
        let destroy_status = nt_user_destroy_window(parent);
        assert_eq!(destroy_status, NtStatus::SUCCESS);

        {
            let state = lock_win32k_state();
            assert!(!state.windows.contains_key(&parent));
            assert!(!state.windows.contains_key(&child));
        }
    }

    #[test]
    fn test_show_window_and_visibility() {
        let mut proc = make_test_proc();

        let (_, hwnd) = nt_user_create_window_ex(
            &mut proc,
            0,
            "HiddenClass",
            "Hidden Window",
            WS_OVERLAPPEDWINDOW, // Not WS_VISIBLE
            10, 10, 400, 300,
            None,
        );

        // Initially hidden
        {
            let state = lock_win32k_state();
            assert!(!state.windows.get(&hwnd).unwrap().visible);
        }

        // Show window
        let (s, prev) = nt_user_show_window(hwnd, SW_SHOW);
        assert_eq!(s, NtStatus::SUCCESS);
        assert!(!prev);

        {
            let state = lock_win32k_state();
            assert!(state.windows.get(&hwnd).unwrap().visible);
        }

        // Hide window
        let (s2, prev2) = nt_user_show_window(hwnd, SW_HIDE);
        assert_eq!(s2, NtStatus::SUCCESS);
        assert!(prev2);

        {
            let state = lock_win32k_state();
            assert!(!state.windows.get(&hwnd).unwrap().visible);
        }
    }

    #[test]
    fn test_set_window_pos_and_zorder() {
        let mut proc = make_test_proc();

        let (_, win1) = nt_user_create_window_ex(
            &mut proc, 0, "Win1", "Win1", WS_OVERLAPPEDWINDOW | WS_VISIBLE, 0, 0, 200, 200, None,
        );
        let (_, win2) = nt_user_create_window_ex(
            &mut proc, 0, "Win2", "Win2", WS_OVERLAPPEDWINDOW | WS_VISIBLE, 10, 10, 200, 200, None,
        );

        // Move and resize win1
        let status = nt_user_set_window_pos(win1, HWND(HWND_TOP), 50, 60, 300, 400, 0);
        assert_eq!(status, NtStatus::SUCCESS);

        let state = lock_win32k_state();
        let w1 = state.windows.get(&win1).unwrap();
        assert_eq!(w1.x, 50);
        assert_eq!(w1.y, 60);
        assert_eq!(w1.width, 300);
        assert_eq!(w1.height, 400);

        // win1 brought to top -> win1 is ahead of win2
        let p1 = state.window_tree.z_order.iter().position(|&h| h == win1).unwrap();
        let p2 = state.window_tree.z_order.iter().position(|&h| h == win2).unwrap();
        assert!(p1 < p2);
        drop(state);

        // Send win1 to bottom -> win1 is behind win2
        let status2 = nt_user_set_window_pos(win1, HWND(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        assert_eq!(status2, NtStatus::SUCCESS);

        let state2 = lock_win32k_state();
        let p1_after = state2.window_tree.z_order.iter().position(|&h| h == win1).unwrap();
        let p2_after = state2.window_tree.z_order.iter().position(|&h| h == win2).unwrap();
        assert!(p1_after > p2_after);
    }

    #[test]
    fn test_thread_message_queue_lifecycle() {
        let mut proc = make_test_proc();

        let (_, hwnd) = nt_user_create_window_ex(
            &mut proc, 0, "MsgTest", "MsgTest", WS_OVERLAPPEDWINDOW, 0, 0, 100, 100, None,
        );

        // 1. Post Custom Messages
        let p1 = nt_user_post_message(hwnd, WM_KEYDOWN, 0x41, 0); // Key 'A'
        let p2 = nt_user_post_message(hwnd, WM_CLOSE, 0, 0);
        assert_eq!(p1, NtStatus::SUCCESS);
        assert_eq!(p2, NtStatus::SUCCESS);

        // 2. PeekMessage with PM_NOREMOVE
        let mut msg_out = MSG::default();
        let s_peek = nt_user_peek_message(&proc, &mut msg_out as *mut MSG as u64, hwnd, 0, 0, PM_NOREMOVE);
        assert_eq!(s_peek, NtStatus(1));
        // WM_CREATE was posted during CreateWindowEx, so first message is WM_CREATE
        assert_eq!(msg_out.message, WM_CREATE);

        // 3. GetMessage (pops WM_CREATE)
        let s_get1 = nt_user_get_message(&proc, &mut msg_out as *mut MSG as u64, hwnd, 0, 0);
        assert_eq!(s_get1, NtStatus(1));
        assert_eq!(msg_out.message, WM_CREATE);

        // 4. GetMessage (pops WM_KEYDOWN)
        let s_get2 = nt_user_get_message(&proc, &mut msg_out as *mut MSG as u64, hwnd, 0, 0);
        assert_eq!(s_get2, NtStatus(1));
        assert_eq!(msg_out.message, WM_KEYDOWN);
        assert_eq!(msg_out.wParam, 0x41);

        // 5. DispatchMessage for WM_CLOSE
        let s_get3 = nt_user_get_message(&proc, &mut msg_out as *mut MSG as u64, hwnd, 0, 0);
        assert_eq!(s_get3, NtStatus(1));
        assert_eq!(msg_out.message, WM_CLOSE);

        let (disp_status, _) = nt_user_dispatch_message(&msg_out as *const MSG as u64);
        assert_eq!(disp_status, NtStatus::SUCCESS);

        // Dispatching WM_CLOSE should post WM_DESTROY
        let s_get4 = nt_user_get_message(&proc, &mut msg_out as *mut MSG as u64, hwnd, 0, 0);
        assert_eq!(s_get4, NtStatus(1));
        assert_eq!(msg_out.message, WM_DESTROY);
    }

    #[test]
    fn test_gdi_device_contexts_and_bitmaps() {
        // 1. Create Compatible DC
        let (s_dc, mem_dc) = nt_gdi_create_compatible_dc(HDC::SCREEN);
        assert_eq!(s_dc, NtStatus::SUCCESS);
        assert!(!mem_dc.is_null());

        // 2. Create Compatible Bitmap
        let (s_bmp, hbmp) = nt_gdi_create_compatible_bitmap(mem_dc, 320, 240);
        assert_eq!(s_bmp, NtStatus::SUCCESS);
        assert!(!hbmp.is_null());

        // 3. Select Bitmap into DC
        let (s_sel, prev_bmp) = nt_gdi_select_bitmap(mem_dc, hbmp);
        assert_eq!(s_sel, NtStatus::SUCCESS);
        assert_eq!(prev_bmp, HBITMAP::NULL);

        // Verify DC dimensions updated to selected bitmap
        {
            let state = lock_win32k_state();
            let dc = state.device_contexts.get(&mem_dc).unwrap();
            assert_eq!(dc.width, 320);
            assert_eq!(dc.height, 240);
            assert_eq!(dc.selected_bitmap, Some(hbmp));
        }

        // 4. Delete DC
        let s_del = nt_gdi_delete_dc(mem_dc);
        assert_eq!(s_del, NtStatus::SUCCESS);
    }

    #[test]
    fn test_gdi_drawing_primitives() {
        let (_, hdc) = nt_gdi_create_compatible_dc(HDC::SCREEN);
        let (_, hbmp) = nt_gdi_create_compatible_bitmap(hdc, 200, 200);
        let _ = nt_gdi_select_bitmap(hdc, hbmp);

        // 1. Rectangle
        let s_rect = nt_gdi_rectangle(hdc, 10, 10, 50, 50, COLOR_RED);
        assert_eq!(s_rect, NtStatus::SUCCESS);

        // Check border pixel
        let (s_pix, color) = nt_gdi_get_pixel(hdc, 10, 10);
        assert_eq!(s_pix, NtStatus::SUCCESS);
        assert_eq!(color, COLOR_RED);

        // 2. FillRect
        let s_fill = nt_gdi_fill_rect(hdc, 60, 60, 80, 80, COLOR_GREEN);
        assert_eq!(s_fill, NtStatus::SUCCESS);

        let (_, fill_pixel) = nt_gdi_get_pixel(hdc, 70, 70);
        assert_eq!(fill_pixel, COLOR_GREEN);

        // 3. Text output
        let test_text = "WIN32K";
        let s_text = nt_gdi_ext_text_out_w(
            hdc,
            0,
            0,
            ETO_OPAQUE,
            test_text.as_ptr() as u64,
            test_text.len() as u32,
        );
        assert_eq!(s_text, NtStatus::SUCCESS);

        // 4. BitBlt (Source memory DC to Destination screen DC)
        let (_, dest_dc) = nt_gdi_create_compatible_dc(HDC::SCREEN);
        let s_blt = nt_gdi_bit_blt(dest_dc, 0, 0, 100, 100, hdc, 0, 0, SRCCOPY);
        assert_eq!(s_blt, NtStatus::SUCCESS);

        let (_, blt_pixel) = nt_gdi_get_pixel(dest_dc, 70, 70);
        assert_eq!(blt_pixel, COLOR_GREEN);

        // Clean up
        let _ = nt_gdi_delete_dc(hdc);
        let _ = nt_gdi_delete_dc(dest_dc);
    }

    #[test]
    fn test_raster_operations() {
        let (_, dc1) = nt_gdi_create_compatible_dc(HDC::SCREEN);
        let (_, dc2) = nt_gdi_create_compatible_dc(HDC::SCREEN);

        let _ = nt_gdi_set_pixel(dc1, 5, 5, 0x00FF0000); // Red
        let _ = nt_gdi_set_pixel(dc2, 5, 5, 0x0000FF00); // Green

        // SRCPAINT (OR)
        let _ = nt_gdi_bit_blt(dc1, 5, 5, 1, 1, dc2, 5, 5, SRCPAINT);
        let (_, res) = nt_gdi_get_pixel(dc1, 5, 5);
        assert_eq!(res, 0x00FFFF00); // Red | Green = Yellow

        // WHITENESS
        let _ = nt_gdi_bit_blt(dc1, 5, 5, 1, 1, dc2, 5, 5, WHITENESS);
        let (_, white_res) = nt_gdi_get_pixel(dc1, 5, 5);
        assert_eq!(white_res, COLOR_WHITE);

        let _ = nt_gdi_delete_dc(dc1);
        let _ = nt_gdi_delete_dc(dc2);
    }

    #[test]
    fn test_win32k_ssdt_dispatcher_dispatch() {
        let mut proc = make_test_proc();

        // 1. Dispatch NtUserCreateWindowEx
        let class_name = "Button\0";
        let title_name = "Start\0";
        let status = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_USER_CREATE_WINDOW_EX,
            WS_EX_APPWINDOW as u64,
            class_name.as_ptr() as u64,
            title_name.as_ptr() as u64,
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u64,
            0,
            0,
        );
        assert!(status.is_success());
        let hwnd = HWND(status.0 as usize);

        // 2. Dispatch NtUserShowWindow
        let show_status = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_USER_SHOW_WINDOW,
            hwnd.0 as u64,
            SW_SHOW as u64,
            0, 0, 0, 0,
        );
        assert!(show_status.is_success());

        // 3. Dispatch NtGdiCreateCompatibleDC
        let dc_status = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_GDI_CREATE_COMPATIBLE_DC,
            0, 0, 0, 0, 0, 0,
        );
        assert!(dc_status.is_success());
        let hdc = HDC(dc_status.0 as usize);

        // 4. Dispatch NtGdiRectangle
        let rect_status = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_GDI_RECTANGLE,
            hdc.0 as u64,
            10, 10, 50, 50,
            COLOR_BLUE as u64,
        );
        assert!(rect_status.is_success());

        // 5. Dispatch NtGdiDeleteDC
        let del_status = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_GDI_DELETE_DC,
            hdc.0 as u64,
            0, 0, 0, 0, 0,
        );
        assert_eq!(del_status, NtStatus::SUCCESS);

        // 6. Dispatch NtUserDestroyWindow
        let destroy_status = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_USER_DESTROY_WINDOW,
            hwnd.0 as u64,
            0, 0, 0, 0, 0,
        );
        assert_eq!(destroy_status, NtStatus::SUCCESS);
    }

    #[test]
    fn test_create_window_with_params_struct() {
        let mut proc = make_test_proc();
        let class_str = "DialogClass\0";
        let title_str = "Settings\0";

        let params = CreateWindowParams {
            ex_style: WS_EX_TOOLWINDOW,
            class_name_ptr: class_str.as_ptr() as u64,
            window_name_ptr: title_str.as_ptr() as u64,
            style: WS_POPUP | WS_VISIBLE,
            x: 120,
            y: 150,
            width: 400,
            height: 300,
            parent_hwnd: 0,
            menu_handle: 0,
            instance_handle: 0,
            param: 0,
        };

        let status = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_USER_CREATE_WINDOW_EX,
            &params as *const CreateWindowParams as u64,
            0, 0, 0, 0, 0,
        );
        assert!(status.is_success());
        let hwnd = HWND(status.0 as usize);

        let state = lock_win32k_state();
        let win = state.windows.get(&hwnd).expect("Dialog window must exist");
        assert_eq!(win.class_name, "DialogClass");
        assert_eq!(win.title, "Settings");
        assert_eq!(win.x, 120);
        assert_eq!(win.y, 150);
        assert_eq!(win.width, 400);
        assert_eq!(win.height, 300);
        drop(state);

        let _ = nt_user_destroy_window(hwnd);
    }

    #[test]
    fn test_get_client_and_window_rect() {
        let mut proc = make_test_proc();
        let (_, hwnd) = nt_user_create_window_ex(
            &mut proc, 0, "RectClass", "Rect Test", WS_OVERLAPPEDWINDOW, 100, 200, 640, 480, None,
        );

        // Window rect
        let mut win_rc = RECT::default();
        let s_wrc = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_USER_GET_WINDOW_RECT,
            hwnd.0 as u64,
            &mut win_rc as *mut RECT as u64,
            0, 0, 0, 0,
        );
        assert_eq!(s_wrc, NtStatus::SUCCESS);
        assert_eq!(win_rc.left, 100);
        assert_eq!(win_rc.top, 200);
        assert_eq!(win_rc.right, 740);
        assert_eq!(win_rc.bottom, 680);

        // Client rect
        let mut cl_rc = RECT::default();
        let s_crc = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_USER_GET_CLIENT_RECT,
            hwnd.0 as u64,
            &mut cl_rc as *mut RECT as u64,
            0, 0, 0, 0,
        );
        assert_eq!(s_crc, NtStatus::SUCCESS);
        assert_eq!(cl_rc.left, 0);
        assert_eq!(cl_rc.top, 0);
        assert!(cl_rc.right > 0);
        assert!(cl_rc.bottom > 0);

        let _ = nt_user_destroy_window(hwnd);
    }

    #[test]
    fn test_find_window_and_desktop_window() {
        let mut proc = make_test_proc();

        // 1. Get Desktop Window
        let desk_status = dispatch_win32k_syscall(&mut proc, SYS_NT_USER_GET_DESKTOP_WINDOW, 0, 0, 0, 0, 0, 0);
        assert!(desk_status.is_success());
        assert_eq!(desk_status.0, HWND::DESKTOP.0 as i32);

        // 2. Create target window
        let (_, target) = nt_user_create_window_ex(
            &mut proc, 0, "UniqueClassXYZ", "UniqueWindowTitle123", WS_OVERLAPPEDWINDOW, 0, 0, 100, 100, None,
        );

        // 3. Find Window by class and title
        let class_name = "UniqueClassXYZ\0";
        let title_name = "UniqueWindowTitle123\0";
        let find_status = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_USER_FIND_WINDOW_EX,
            0,
            class_name.as_ptr() as u64,
            title_name.as_ptr() as u64,
            0, 0, 0,
        );
        assert!(find_status.is_success());
        assert_eq!(find_status.0, target.0 as i32);

        let _ = nt_user_destroy_window(target);
    }

    #[test]
    fn test_gdi_solid_brush_and_lineto() {
        let mut proc = make_test_proc();
        let (_, hdc) = nt_gdi_create_compatible_dc(HDC::SCREEN);

        // 1. Create Solid Brush
        let brush_status = dispatch_win32k_syscall(&mut proc, SYS_NT_GDI_CREATE_SOLID_BRUSH, COLOR_RED as u64, 0, 0, 0, 0, 0);
        assert!(brush_status.is_success());
        let hbrush = HBRUSH(brush_status.0 as usize);

        // 2. Select Brush
        let sel_status = dispatch_win32k_syscall(&mut proc, SYS_NT_GDI_SELECT_BRUSH, hdc.0 as u64, hbrush.0 as u64, 0, 0, 0, 0);
        assert!(sel_status.is_success());

        // 3. LineTo
        let line_status = dispatch_win32k_syscall(&mut proc, SYS_NT_GDI_LINE_TO, hdc.0 as u64, 20, 20, COLOR_BLUE as u64, 0, 0);
        assert_eq!(line_status, NtStatus::SUCCESS);

        let _ = nt_gdi_delete_dc(hdc);
    }

    #[test]
    fn test_bitblt_with_params_struct_and_srcand() {
        let (_, dc_src) = nt_gdi_create_compatible_dc(HDC::SCREEN);
        let (_, dc_dst) = nt_gdi_create_compatible_dc(HDC::SCREEN);

        // Draw initial colors
        let _ = nt_gdi_set_pixel(dc_dst, 10, 10, 0x00FF00FF); // Magenta (Red + Blue)
        let _ = nt_gdi_set_pixel(dc_src, 0, 0, 0x00FFFF00);   // Yellow (Red + Green)

        let params = BitBltParams {
            hdc_dest: dc_dst.0,
            x_dest: 10,
            y_dest: 10,
            width: 1,
            height: 1,
            hdc_src: dc_src.0,
            x_src: 0,
            y_src: 0,
            rop: SRCAND,
        };

        let mut proc = make_test_proc();
        let blt_status = dispatch_win32k_syscall(
            &mut proc,
            SYS_NT_GDI_BIT_BLT,
            &params as *const BitBltParams as u64,
            0, 0, 0, 0, 0,
        );
        assert_eq!(blt_status, NtStatus::SUCCESS);

        // 0x00FF00FF & 0x00FFFF00 = 0x00FF0000 (Red)
        let (_, pixel) = nt_gdi_get_pixel(dc_dst, 10, 10);
        assert_eq!(pixel, 0x00FF0000);

        let _ = nt_gdi_delete_dc(dc_src);
        let _ = nt_gdi_delete_dc(dc_dst);
    }

    #[test]
    fn test_syscall_router_win32k_integration() {
        use crate::syscall::SyscallRouter;
        use crate::trap::TrapFrame;

        let mut proc = make_test_proc();
        let mut tf = TrapFrame::default();

        // Calling NtUserGetDesktopWindow (0x100F) via SyscallRouter
        tf.rax = SYS_NT_USER_GET_DESKTOP_WINDOW;
        let ret = SyscallRouter::dispatch(&mut proc, &mut tf);
        assert_eq!(ret, HWND::DESKTOP.0 as u64);
        assert_eq!(tf.rax, HWND::DESKTOP.0 as u64);
    }
}
